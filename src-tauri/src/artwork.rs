use crate::config::{AppConfig, GameArtwork};
use crate::{sync, AppState};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

mod wikipedia;
use wikipedia::WikipediaClient;

const API_BASE: &str = "https://store.steampowered.com/api/";
const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const PAGE_SIZE: usize = 50;

#[derive(Default)]
pub struct ArtworkService {
    operation: tokio::sync::Mutex<()>,
    retry_after: Mutex<Option<Instant>>,
    details: Mutex<HashMap<u64, (Instant, StoreGameDetails)>>,
    wikipedia_retry_after: Mutex<Option<Instant>>,
    wikipedia_pages: Mutex<HashMap<u64, (Instant, wikipedia::WikipediaPage)>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtworkSource {
    #[default]
    Steam,
    Wikipedia,
}

impl ArtworkSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Steam => "steam",
            Self::Wikipedia => "wikipedia",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArtworkGame {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub source: ArtworkSource,
}

#[derive(Debug, Serialize)]
pub struct ArtworkSearchResult {
    games: Vec<ArtworkGame>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ArtworkAuthor {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArtworkImage {
    pub id: u64,
    pub url: String,
    pub thumb: String,
    pub width: u32,
    pub height: u32,
    pub author: ArtworkAuthor,
}

#[derive(Deserialize)]
struct StoreSearch {
    #[serde(default)]
    items: Vec<ArtworkGame>,
}

#[derive(Deserialize)]
struct StoreDetailsResponse {
    success: bool,
    data: Option<StoreGameDetails>,
}

#[derive(Debug, Clone, Deserialize)]
struct StoreGameDetails {
    name: String,
    #[serde(default)]
    header_image: String,
    #[serde(default)]
    background_raw: String,
    #[serde(default)]
    screenshots: Vec<StoreScreenshot>,
    #[serde(default)]
    developers: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct StoreScreenshot {
    id: u64,
    path_full: String,
    path_thumbnail: String,
}

struct StoreClient<'a> {
    http: reqwest::Client,
    base: url::Url,
    service: &'a ArtworkService,
}

impl<'a> StoreClient<'a> {
    fn new(service: &'a ArtworkService) -> Result<Self, String> {
        Ok(Self {
            http: http_client()?,
            base: url::Url::parse(API_BASE).map_err(|e| e.to_string())?,
            service,
        })
    }

    async fn get<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        query: &[(&str, &str)],
    ) -> Result<T, String> {
        if self
            .service
            .retry_after
            .lock()
            .unwrap()
            .is_some_and(|time| time > Instant::now())
        {
            return Err("Steam dočasne obmedzuje požiadavky. Skús to o chvíľu znova.".into());
        }
        let url = self
            .base
            .join(endpoint)
            .map_err(|_| "Neplatná adresa katalógu Steam.".to_string())?;
        let response = self.http.get(url).query(query).send().await.map_err(|_| {
            *self.service.retry_after.lock().unwrap() =
                Some(Instant::now() + Duration::from_secs(30));
            "Katalóg Steam sa nedá kontaktovať. Skontroluj internetové pripojenie.".to_string()
        })?;
        if response.status().as_u16() == 429 {
            let seconds = response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(60)
                .clamp(1, 3600);
            *self.service.retry_after.lock().unwrap() =
                Some(Instant::now() + Duration::from_secs(seconds));
            return Err(format!(
                "Steam dosiahol limit požiadaviek. Skús znova o {seconds} sekúnd."
            ));
        }
        if !response.status().is_success() {
            *self.service.retry_after.lock().unwrap() =
                Some(Instant::now() + Duration::from_secs(30));
            return Err(format!(
                "Katalóg Steam je dočasne nedostupný (HTTP {}).",
                response.status().as_u16()
            ));
        }
        let bytes = limited_body(response, 4 * 1024 * 1024).await?;
        serde_json::from_slice(&bytes).map_err(|_| "Katalóg Steam vrátil neplatnú odpoveď.".into())
    }

    async fn search(&self, name: &str) -> Result<Vec<ArtworkGame>, String> {
        let name = name.trim();
        validate_query(name)?;
        let mut result: StoreSearch = self
            .get(
                "storesearch/",
                &[("term", name), ("l", "english"), ("cc", "us")],
            )
            .await?;
        result.items.truncate(20);
        Ok(result.items)
    }

    async fn details(&self, id: u64) -> Result<StoreGameDetails, String> {
        if let Some((time, details)) = self.service.details.lock().unwrap().get(&id) {
            if time.elapsed() < Duration::from_secs(15 * 60) {
                return Ok(details.clone());
            }
        }
        let id_string = id.to_string();
        let mut result: HashMap<String, StoreDetailsResponse> = self
            .get(
                "appdetails",
                &[("appids", &id_string), ("l", "english"), ("cc", "us")],
            )
            .await?;
        let response = result
            .remove(&id_string)
            .filter(|response| response.success)
            .ok_or("Hra nie je dostupná vo verejnom katalógu Steam.")?;
        let details = response
            .data
            .ok_or("V katalógu Steam chýbajú údaje o hre.")?;
        let mut cache = self.service.details.lock().unwrap();
        if cache.len() >= 128 {
            cache.clear();
        }
        cache.insert(id, (Instant::now(), details.clone()));
        Ok(details)
    }

    #[cfg(test)]
    async fn images(&self, id: u64, kind: &str, page: u32) -> Result<Vec<ArtworkImage>, String> {
        let details = self.details(id).await?;
        images_from_details(&details, kind, page)
    }
}

fn images_from_details(
    details: &StoreGameDetails,
    kind: &str,
    page: u32,
) -> Result<Vec<ArtworkImage>, String> {
    let author = details
        .developers
        .first()
        .cloned()
        .unwrap_or_else(|| "Steam".into());
    let mut images = Vec::new();
    match kind {
        "grid" => {
            for (index, url) in [&details.header_image, &details.background_raw]
                .into_iter()
                .enumerate()
            {
                images.push(ArtworkImage {
                    id: index as u64 + 1,
                    url: url.clone(),
                    thumb: url.clone(),
                    width: 0,
                    height: 0,
                    author: ArtworkAuthor {
                        name: author.clone(),
                    },
                });
            }
        }
        "hero" => {
            for screenshot in &details.screenshots {
                images.push(ArtworkImage {
                    id: screenshot.id.saturating_add(100),
                    url: screenshot.path_full.clone(),
                    thumb: screenshot.path_thumbnail.clone(),
                    width: 0,
                    height: 0,
                    author: ArtworkAuthor {
                        name: author.clone(),
                    },
                });
            }
        }
        _ => return Err("Neplatný typ obrázka.".into()),
    }
    images.retain(|image| valid_media_url(&image.url) && valid_media_url(&image.thumb));
    Ok(images
        .into_iter()
        .skip((page as usize).saturating_mul(PAGE_SIZE))
        .take(PAGE_SIZE)
        .collect())
}

fn lookup_name(name: &str) -> String {
    format!("steam:{name}")
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!(
            "BackupSync/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/ValentinSK0/backup-sync)"
        ))
        .build()
        .map_err(|_| "Nepodarilo sa pripraviť pripojenie ku katalógu obrázkov.".into())
}

fn valid_media_url(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|url| {
        let host = url.host_str().unwrap_or_default();
        let trusted = host.ends_with(".steamstatic.com")
            || host == "steamcdn-a.akamaihd.net"
            || host == "cdn2.steamgriddb.com"
            || host == "upload.wikimedia.org";
        url.scheme() == "https"
            && trusted
            && url.port().is_none()
            && url.username().is_empty()
            && url.password().is_none()
    })
}

async fn limited_body(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|size| size > limit as u64)
    {
        return Err("Odpoveď servera prekračuje povolenú veľkosť.".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Sťahovanie zo servera bolo prerušené.".to_string())?
    {
        if bytes.len() + chunk.len() > limit {
            return Err("Odpoveď servera prekračuje povolenú veľkosť.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn image_mime(bytes: &[u8]) -> Result<&'static str, String> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Ok("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Ok("image/jpeg")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Ok("image/webp")
    } else {
        Err("Server vrátil nepodporovaný obrázok.".into())
    }
}

async fn cached_image(directory: &Path, url: &str) -> Result<String, String> {
    if !valid_media_url(url) {
        return Err("Katalóg vrátil nepovolenú adresu obrázka.".into());
    }
    let path = cache_path(directory, url);
    if let Some(cached) = read_cached_image(&path).await {
        return Ok(cached);
    }
    let response = http_client()?
        .get(url)
        .send()
        .await
        .map_err(|_| "Obrázok sa nepodarilo stiahnuť. Skontroluj pripojenie.".to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "Obrázok sa nepodarilo stiahnuť (HTTP {}).",
            response.status().as_u16()
        ));
    }
    let bytes = limited_body(response, MAX_IMAGE_BYTES).await?;
    let data_url = format!(
        "data:{};base64,{}",
        image_mime(&bytes)?,
        STANDARD.encode(&bytes)
    );
    let saved = data_url.clone();
    tokio::task::spawn_blocking(move || crate::config::write_atomic(&path, saved.as_bytes()))
        .await
        .map_err(|_| "Obrázok sa nepodarilo uložiť.".to_string())??;
    Ok(data_url)
}

fn cache_path(directory: &Path, url: &str) -> PathBuf {
    let hash = format!("{:x}", Sha256::digest(url.as_bytes()));
    directory.join("artwork").join(format!("{hash}.data-url"))
}

async fn read_cached_image(path: &Path) -> Option<String> {
    let size = tokio::fs::metadata(path).await.ok()?.len();
    if size > (MAX_IMAGE_BYTES * 4 / 3 + 64) as u64 {
        return None;
    }
    let data = tokio::fs::read_to_string(path).await.ok()?;
    let (header, encoded) = data.split_once(',')?;
    let decoded = STANDARD.decode(encoded).ok()?;
    let mime = image_mime(&decoded).ok()?;
    if header != format!("data:{mime};base64") {
        return None;
    }
    Some(data)
}

fn normalized_name(name: &str) -> String {
    name.chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn exact_match<'a>(games: &'a [ArtworkGame], name: &str) -> Option<&'a ArtworkGame> {
    let mut exact = games.iter().filter(|game| game.name == name.trim());
    if let Some(first) = exact.next() {
        if exact.next().is_none() {
            return Some(first);
        }
    }
    let name = normalized_name(name);
    let mut matches = games.iter().filter(|game| {
        let title = match game.source {
            ArtworkSource::Steam => game.name.as_str(),
            ArtworkSource::Wikipedia => wikipedia::matching_title(&game.name),
        };
        normalized_name(title) == name
    });
    let first = matches.next()?;
    // Ambiguous titles (for example a remake and an original) need a manual selection.
    if matches.next().is_some() {
        None
    } else {
        Some(first)
    }
}

#[tauri::command]
pub async fn search_artwork_games(
    app: AppHandle,
    query: String,
) -> Result<ArtworkSearchResult, String> {
    let state = app.state::<AppState>();
    validate_query(&query)?;
    let (steam, wikipedia) = tokio::join!(
        search_source(&state.artwork, ArtworkSource::Steam, &query),
        search_source(&state.artwork, ArtworkSource::Wikipedia, &query)
    );
    let mut games = Vec::new();
    let mut warnings = Vec::new();
    for result in [steam, wikipedia] {
        match result {
            Ok(found) => games.extend(found),
            Err(error) => warnings.push(error),
        }
    }
    if games.is_empty() && !warnings.is_empty() {
        return Err(warnings.join(" "));
    }
    Ok(ArtworkSearchResult { games, warnings })
}

fn validate_query(name: &str) -> Result<(), String> {
    if name.trim().is_empty() || name.chars().count() > 200 {
        return Err("Zadaj názov hry od 1 do 200 znakov.".into());
    }
    Ok(())
}

async fn search_source(
    service: &ArtworkService,
    source: ArtworkSource,
    query: &str,
) -> Result<Vec<ArtworkGame>, String> {
    match source {
        ArtworkSource::Steam => StoreClient::new(service)?.search(query).await,
        ArtworkSource::Wikipedia => WikipediaClient::new(service)?.search(query).await,
    }
}

async fn catalogue_images(
    service: &ArtworkService,
    source: ArtworkSource,
    game_id: u64,
    kind: &str,
    page: u32,
) -> Result<(String, Vec<ArtworkImage>), String> {
    match source {
        ArtworkSource::Steam => {
            let details = StoreClient::new(service)?.details(game_id).await?;
            let images = images_from_details(&details, kind, page)?;
            Ok((details.name, images))
        }
        ArtworkSource::Wikipedia => {
            let details = WikipediaClient::new(service)?.details(game_id).await?;
            let images = details.images(kind, page)?;
            Ok((details.title, images))
        }
    }
}

#[tauri::command]
pub async fn list_artwork_images(
    app: AppHandle,
    game_id: u64,
    kind: String,
    page: u32,
    source: Option<ArtworkSource>,
) -> Result<Vec<ArtworkImage>, String> {
    let state = app.state::<AppState>();
    catalogue_images(
        &state.artwork,
        source.unwrap_or_default(),
        game_id,
        &kind,
        page,
    )
    .await
    .map(|(_, images)| images)
}

async fn automatic_artwork(
    service: &ArtworkService,
    directory: &Path,
    name: &str,
) -> Result<Option<(GameArtwork, String)>, String> {
    let mut errors = Vec::new();
    for source in [ArtworkSource::Steam, ArtworkSource::Wikipedia] {
        let games = match search_source(service, source, name).await {
            Ok(games) => games,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let Some(matched) = exact_match(&games, name) else {
            continue;
        };
        for kind in ["grid", "hero"] {
            let images = match catalogue_images(service, source, matched.id, kind, 0).await {
                Ok((_, images)) => images,
                Err(error) => {
                    errors.push(error);
                    break;
                }
            };
            for image in images.into_iter().take(3) {
                match cached_image(directory, &image.url).await {
                    Ok(data) => {
                        return Ok(Some((
                            GameArtwork {
                                source: source.as_str().into(),
                                game_id: matched.id,
                                game_name: matched.name.clone(),
                                image_id: image.id,
                                kind: kind.into(),
                                url: image.url,
                                author: image.author.name,
                                selected_manually: false,
                            },
                            data,
                        )));
                    }
                    Err(error) => errors.push(error),
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(None)
    } else {
        Err(errors.join(" "))
    }
}

#[tauri::command]
pub async fn load_game_artwork(
    app: AppHandle,
    id: String,
    automatic: bool,
) -> Result<Option<String>, String> {
    let state = app.state::<AppState>();
    // Cached covers stay available while other games wait for the public catalogue.
    if let Some(artwork) = sync::snapshot(&app)
        .await
        .games
        .iter()
        .find(|game| game.id == id)
        .and_then(|game| game.artwork.as_ref())
    {
        if valid_media_url(&artwork.url) {
            if let Some(cached) =
                read_cached_image(&cache_path(&state.state_dir(), &artwork.url)).await
            {
                return Ok(Some(cached));
            }
        }
    }
    let _guard = state.artwork.operation.lock().await;
    let config = sync::snapshot(&app).await;
    let game = config
        .games
        .iter()
        .find(|game| game.id == id)
        .ok_or("Hra sa nenašla.")?;
    if let Some(artwork) = &game.artwork {
        return cached_image(&state.state_dir(), &artwork.url)
            .await
            .map(Some);
    }
    let lookup = lookup_name(&game.name);
    if !automatic || game.artwork_checked_name.as_ref() == Some(&lookup) {
        return Ok(None);
    }
    let resolved = automatic_artwork(&state.artwork, &state.state_dir(), &game.name).await?;
    let (artwork, data_url) = match resolved {
        Some((artwork, data)) => (Some(artwork), Some(data)),
        None => (None, None),
    };
    let mut applied = false;
    sync::with_config(&app, |config| {
        if let Some(current) = config.games.iter_mut().find(|current| current.id == id) {
            if current.name == game.name && current.artwork.is_none() {
                current.artwork = artwork;
                current.artwork_checked_name = Some(lookup);
                applied = true;
            }
        }
    })
    .await?;
    Ok(if applied { data_url } else { None })
}

#[tauri::command]
pub async fn select_game_artwork(
    app: AppHandle,
    id: String,
    game_id: u64,
    kind: String,
    page: u32,
    image_id: u64,
    source: Option<ArtworkSource>,
) -> Result<AppConfig, String> {
    let state = app.state::<AppState>();
    let _guard = state.artwork.operation.lock().await;
    if !sync::snapshot(&app)
        .await
        .games
        .iter()
        .any(|game| game.id == id)
    {
        return Err("Hra sa nenašla.".into());
    }
    let source = source.unwrap_or_default();
    let (game_name, images) =
        catalogue_images(&state.artwork, source, game_id, &kind, page).await?;
    let image = images
        .into_iter()
        .find(|image| image.id == image_id)
        .ok_or("Vybraný obrázok už nie je dostupný. Načítaj galériu znova.")?;
    cached_image(&state.state_dir(), &image.url).await?;
    sync::with_config(&app, |config| {
        if let Some(game) = config.games.iter_mut().find(|game| game.id == id) {
            game.artwork = Some(GameArtwork {
                source: source.as_str().into(),
                game_id,
                game_name,
                image_id,
                kind,
                url: image.url,
                author: image.author.name,
                selected_manually: true,
            });
            game.artwork_checked_name = Some(lookup_name(&game.name));
        }
    })
    .await
}

#[tauri::command]
pub async fn clear_game_artwork(app: AppHandle, id: String) -> Result<AppConfig, String> {
    let state = app.state::<AppState>();
    let _guard = state.artwork.operation.lock().await;
    sync::with_config(&app, |config| {
        if let Some(game) = config.games.iter_mut().find(|game| game.id == id) {
            game.artwork = None;
            game.artwork_checked_name = Some(lookup_name(&game.name));
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    pub(super) fn mock_api(
        status: u16,
        headers: &str,
        body: &str,
    ) -> (url::Url, std::thread::JoinHandle<String>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base =
            url::Url::parse(&format!("http://{}/api/", listener.local_addr().unwrap())).unwrap();
        let response = format!("HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nContent-Type: application/json\r\n{headers}Connection: close\r\n\r\n{body}", body.len());
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let count = stream.read(&mut buffer).unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..count]);
            }
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8(request).unwrap()
        });
        (base, worker)
    }

    pub(super) fn details_fixture() -> StoreGameDetails {
        serde_json::from_value(serde_json::json!({
            "name": "Mafia: The Old Country",
            "header_image": "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/1941540/header.jpg",
            "background_raw": "https://shared.fastly.steamstatic.com/store_item_assets/steam/apps/1941540/background.jpg",
            "developers": ["Hangar 13"],
            "screenshots": [{"id":0,
                "path_full":"https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/1941540/screen.jpg",
                "path_thumbnail":"https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/1941540/screen-thumb.jpg"}]
        })).unwrap()
    }

    #[tokio::test]
    async fn public_search_encodes_name_and_sends_no_credentials_or_cookies() {
        let (base, worker) = mock_api(
            200,
            "",
            r#"{"total":1,"items":[{"id":1941540,"name":"Mafia: The Old Country"}]}"#,
        );
        let service = ArtworkService::default();
        let mut client = StoreClient::new(&service).unwrap();
        client.base = base;
        let results = client.search("Mafia the old country & A/B").await.unwrap();
        assert_eq!(results[0].id, 1941540);
        let request = worker.join().unwrap();
        let target = request
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap();
        let url = url::Url::parse(&format!("http://localhost{target}")).unwrap();
        let query: HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(url.path(), "/api/storesearch/");
        assert_eq!(query.get("term").unwrap(), "Mafia the old country & A/B");
        assert!(!request.to_ascii_lowercase().contains("authorization:"));
        assert!(!request.to_ascii_lowercase().contains("cookie:"));
    }

    #[test]
    fn automatic_matching_handles_mafia_punctuation_but_rejects_dlc_and_sequels() {
        let games = vec![
            ArtworkGame {
                id: 1,
                name: "Mafia: The Old Country - Padrino Pack".into(),
                source: ArtworkSource::Steam,
            },
            ArtworkGame {
                id: 1941540,
                name: "Mafia: The Old Country".into(),
                source: ArtworkSource::Steam,
            },
            ArtworkGame {
                id: 2,
                name: "Portal 2".into(),
                source: ArtworkSource::Steam,
            },
        ];
        assert_eq!(
            exact_match(&games, "Mafia the old country").unwrap().id,
            1941540
        );
        assert!(exact_match(&games, "Portal").is_none());
    }

    #[tokio::test]
    async fn game_details_are_cached_and_reused_for_the_gallery_and_selection() {
        let (base, worker) = mock_api(
            200,
            "",
            r#"{"1941540":{"success":true,"data":{"name":"Mafia: The Old Country","header_image":"https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/1941540/header.jpg"}}}"#,
        );
        let service = ArtworkService::default();
        let mut client = StoreClient::new(&service).unwrap();
        client.base = base;
        assert_eq!(client.images(1941540, "grid", 0).await.unwrap().len(), 1);
        worker.join().unwrap();
        assert_eq!(
            client.details(1941540).await.unwrap().name,
            "Mafia: The Old Country"
        );
        assert!(client.images(1941540, "hero", 0).await.unwrap().is_empty());
    }

    #[test]
    fn gallery_images_come_from_the_selected_game_and_support_both_categories() {
        let details = details_fixture();
        let covers = images_from_details(&details, "grid", 0).unwrap();
        let screenshots = images_from_details(&details, "hero", 0).unwrap();
        assert_eq!(covers.len(), 2);
        assert_eq!(covers[0].id, 1);
        assert_eq!(screenshots.len(), 1);
        assert_eq!(screenshots[0].id, 100);
        assert_eq!(screenshots[0].author.name, "Hangar 13");
        assert!(images_from_details(&details, "grid", u32::MAX)
            .unwrap()
            .is_empty());
        assert!(images_from_details(&details, "other", 0).is_err());
    }

    #[test]
    fn media_addresses_allow_steam_cdns_and_legacy_covers_but_reject_other_hosts() {
        assert!(valid_media_url(
            "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/1/header.jpg"
        ));
        assert!(valid_media_url("https://cdn2.steamgriddb.com/grid/old.png"));
        for url in [
            "http://shared.akamai.steamstatic.com/image.jpg",
            "https://steamstatic.com.attacker.test/image.jpg",
            "https://attacker.test/image.jpg",
            "https://user:password@shared.akamai.steamstatic.com/image.jpg",
            "https://shared.akamai.steamstatic.com:8080/image.jpg",
            "file:///C:/image.jpg",
        ] {
            assert!(!valid_media_url(url), "{url}");
        }
    }

    #[tokio::test]
    async fn rate_limit_blocks_following_requests() {
        let (base, worker) = mock_api(429, "Retry-After: 30\r\n", "{}");
        let service = ArtworkService::default();
        let mut client = StoreClient::new(&service).unwrap();
        client.base = base;
        assert!(client.search("Mafia").await.unwrap_err().contains("30"));
        worker.join().unwrap();
        assert!(client
            .search("Mafia")
            .await
            .unwrap_err()
            .contains("dočasne"));
    }

    #[tokio::test]
    async fn missing_games_and_bad_responses_report_errors_without_login_prompts() {
        for (status, body, expected) in [
            (200, r#"{"1":{"success":false}}"#, "Hra nie je dostupná"),
            (200, "not-json", "neplatnú odpoveď"),
            (403, "private response", "dočasne nedostupný"),
        ] {
            let (base, worker) = mock_api(status, "", body);
            let service = ArtworkService::default();
            let mut client = StoreClient::new(&service).unwrap();
            client.base = base;
            let error = client.details(1).await.unwrap_err();
            assert!(error.contains(expected), "{error}");
            worker.join().unwrap();
        }
    }

    #[tokio::test]
    async fn cached_images_load_offline_and_old_lookup_markers_are_retried() {
        let root =
            std::env::temp_dir().join(format!("backup-sync-artwork-test-{}", uuid::Uuid::new_v4()));
        let url = "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/1/image.png";
        let file = cache_path(&root, url);
        let data = format!(
            "data:image/png;base64,{}",
            STANDARD.encode(b"\x89PNG\r\n\x1a\n")
        );
        crate::config::write_atomic(&file, data.as_bytes()).unwrap();
        assert_eq!(cached_image(&root, url).await.unwrap(), data);
        assert_ne!(lookup_name("Mafia"), "Mafia");
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(root.join("artwork")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn old_configs_and_selected_steamgriddb_images_are_compatible() {
        let config:AppConfig=serde_json::from_str(r#"{"games":[{"id":"old","name":"Mafia","savePath":"C:/Saves","provider":"gdrive","artwork":{"gameId":9,"gameName":"Mafia","imageId":10,"kind":"grid","url":"https://cdn2.steamgriddb.com/grid/image.png","author":"Artist","selectedManually":true}}],"settings":{"steamgriddbApiKeyConfigured":true}}"#).unwrap();
        let art = config.games[0].artwork.as_ref().unwrap();
        assert_eq!(art.source, "steamgriddb");
        assert!(art.selected_manually);
        assert!(!serde_json::to_string(&config)
            .unwrap()
            .contains("steamgriddbApiKeyConfigured"));
    }

    #[tokio::test]
    #[ignore = "Live public Steam catalogue and image download"]
    async fn live_public_steam_artwork_without_login() {
        let service = ArtworkService::default();
        let client = StoreClient::new(&service).unwrap();
        let root =
            std::env::temp_dir().join(format!("backup-sync-steam-live-{}", uuid::Uuid::new_v4()));
        for name in [
            "Mafia the old country",
            "Cyberpunk 2077",
            "ELDEN RING",
            "Dressmaker",
        ] {
            let games = client.search(name).await.unwrap();
            let matched = exact_match(&games, name)
                .unwrap_or_else(|| panic!("No exact match for {name}: {games:?}"));
            let images = client.images(matched.id, "grid", 0).await.unwrap();
            let image = images.first().unwrap();
            let data = cached_image(&root, &image.url).await.unwrap();
            assert!(data.starts_with("data:image/"));
            assert!(data.len() > 1000);
            assert_eq!(cached_image(&root, &image.url).await.unwrap(), data);
            let screenshots = client.images(matched.id, "hero", 0).await.unwrap();
            if let Some(screenshot) = screenshots.first() {
                assert!(cached_image(&root, &screenshot.url).await.unwrap().len() > 1000);
            }
            println!(
                "LIVE OK {name}: Steam ID {}, {} covers, {} screenshots, downloaded {} bytes",
                matched.id,
                images.len(),
                screenshots.len(),
                data.len()
            );
        }
        for entry in std::fs::read_dir(root.join("artwork")).unwrap() {
            std::fs::remove_file(entry.unwrap().path()).unwrap();
        }
        std::fs::remove_dir(root.join("artwork")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
