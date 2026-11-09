use super::{
    http_client, limited_body, valid_media_url, validate_query, ArtworkAuthor, ArtworkGame,
    ArtworkImage, ArtworkService, ArtworkSource,
};
use serde::Deserialize;
use std::time::{Duration, Instant};

const API_URL: &str = "https://en.wikipedia.org/w/api.php";

#[derive(Debug, Clone, Deserialize)]
pub(super) struct WikipediaPage {
    pub pageid: u64,
    pub title: String,
    #[serde(default)]
    index: u32,
    #[serde(default)]
    missing: bool,
    original: Option<WikipediaImage>,
    thumbnail: Option<WikipediaImage>,
}

#[derive(Debug, Clone, Deserialize)]
struct WikipediaImage {
    source: String,
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
struct WikipediaResponse {
    query: Option<WikipediaQuery>,
    error: Option<WikipediaError>,
}

#[derive(Deserialize)]
struct WikipediaQuery {
    #[serde(default)]
    pages: Vec<WikipediaPage>,
}

#[derive(Deserialize)]
struct WikipediaError {
    code: String,
}

impl WikipediaPage {
    pub(super) fn images(&self, kind: &str, page: u32) -> Result<Vec<ArtworkImage>, String> {
        if !matches!(kind, "grid" | "hero") {
            return Err("Neplatný typ obrázka.".into());
        }
        // PageImages supplies the article's main cover, not a screenshot gallery.
        if kind != "grid" || page != 0 || self.missing {
            return Ok(Vec::new());
        }
        let Some(original) = &self.original else {
            return Ok(Vec::new());
        };
        let thumbnail = self.thumbnail.as_ref().unwrap_or(original);
        let Some(url) = image_url(&original.source) else {
            return Ok(Vec::new());
        };
        let Some(thumb) = image_url(&thumbnail.source) else {
            return Ok(Vec::new());
        };
        Ok(vec![ArtworkImage {
            id: self.pageid,
            url,
            thumb,
            width: original.width,
            height: original.height,
            author: ArtworkAuthor {
                name: "Wikipedia".into(),
            },
        }])
    }
}

fn image_url(value: &str) -> Option<String> {
    let mut url = url::Url::parse(value).ok()?;
    if url.host_str() != Some("upload.wikimedia.org") || !valid_media_url(value) {
        return None;
    }
    let path = url.path().to_ascii_lowercase();
    if ![".jpg", ".jpeg", ".png", ".webp"]
        .iter()
        .any(|extension| path.ends_with(extension))
    {
        return None;
    }
    url.set_query(None);
    url.set_fragment(None);
    Some(url.into())
}

pub(super) fn matching_title(title: &str) -> &str {
    if let Some((name, suffix)) = title.rsplit_once(" (") {
        if suffix.ends_with(')')
            && (suffix.contains("video game") || suffix.contains("computer game"))
        {
            return name;
        }
    }
    title
}

pub(super) struct WikipediaClient<'a> {
    http: reqwest::Client,
    base: url::Url,
    service: &'a ArtworkService,
}

impl<'a> WikipediaClient<'a> {
    pub(super) fn new(service: &'a ArtworkService) -> Result<Self, String> {
        Ok(Self {
            http: http_client()?,
            base: url::Url::parse(API_URL).map_err(|error| error.to_string())?,
            service,
        })
    }

    async fn pages(&self, query: &[(&str, &str)]) -> Result<Vec<WikipediaPage>, String> {
        if self
            .service
            .wikipedia_retry_after
            .lock()
            .unwrap()
            .is_some_and(|time| time > Instant::now())
        {
            return Err("Wikipedia dočasne obmedzuje požiadavky. Skús to o chvíľu znova.".into());
        }
        let response = self
            .http
            .get(self.base.clone())
            .query(&[
                ("action", "query"),
                ("format", "json"),
                ("formatversion", "2"),
                ("prop", "pageimages"),
                ("piprop", "original|thumbnail"),
                ("pilicense", "any"),
                ("pithumbsize", "600"),
                ("pilimit", "20"),
                ("maxlag", "5"),
            ])
            .query(query)
            .send()
            .await
            .map_err(|_| {
                *self.service.wikipedia_retry_after.lock().unwrap() =
                    Some(Instant::now() + Duration::from_secs(30));
                "Wikipédiu sa nedá kontaktovať. Skontroluj internetové pripojenie.".to_string()
            })?;
        if !response.status().is_success() {
            let seconds = response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(60)
                .clamp(1, 3600);
            *self.service.wikipedia_retry_after.lock().unwrap() =
                Some(Instant::now() + Duration::from_secs(seconds));
            return Err(format!(
                "Wikipedia je dočasne nedostupná (HTTP {}). Skús znova o {seconds} sekúnd.",
                response.status().as_u16()
            ));
        }
        let bytes = limited_body(response, 4 * 1024 * 1024).await?;
        let result: WikipediaResponse = serde_json::from_slice(&bytes)
            .map_err(|_| "Wikipedia vrátila neplatnú odpoveď.".to_string())?;
        if let Some(error) = result.error {
            *self.service.wikipedia_retry_after.lock().unwrap() =
                Some(Instant::now() + Duration::from_secs(30));
            return Err(format!(
                "Wikipedia požiadavku nespracovala ({}). Skús to o chvíľu znova.",
                error.code
            ));
        }
        let pages = result.query.map(|query| query.pages).unwrap_or_default();
        let mut cache = self.service.wikipedia_pages.lock().unwrap();
        if cache.len() + pages.len() > 128 {
            cache.clear();
        }
        for page in &pages {
            cache.insert(page.pageid, (Instant::now(), page.clone()));
        }
        Ok(pages)
    }

    pub(super) async fn search(&self, name: &str) -> Result<Vec<ArtworkGame>, String> {
        validate_query(name)?;
        let query = format!("{} video game", name.trim());
        let mut pages = self
            .pages(&[
                ("generator", "search"),
                ("gsrsearch", &query),
                ("gsrnamespace", "0"),
                ("gsrlimit", "20"),
            ])
            .await?;
        pages.sort_by_key(|page| page.index);
        let mut games = Vec::new();
        for page in pages {
            if !page.images("grid", 0)?.is_empty() {
                games.push(ArtworkGame {
                    id: page.pageid,
                    name: page.title,
                    source: ArtworkSource::Wikipedia,
                });
            }
        }
        Ok(games)
    }

    pub(super) async fn details(&self, id: u64) -> Result<WikipediaPage, String> {
        if let Some((time, page)) = self.service.wikipedia_pages.lock().unwrap().get(&id) {
            if time.elapsed() < Duration::from_secs(15 * 60) {
                return Ok(page.clone());
            }
        }
        self.pages(&[("pageids", &id.to_string())])
            .await?
            .into_iter()
            .find(|page| page.pageid == id && !page.missing)
            .ok_or_else(|| "Hra už nie je dostupná vo Wikipédii.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page_fixture() -> WikipediaPage {
        serde_json::from_value(serde_json::json!({
            "pageid": 26809,
            "title": "StarCraft (video game)",
            "index": 1,
            "original": {
                "source": "https://upload.wikimedia.org/wikipedia/en/9/93/StarCraft_box_art.jpg?utm_source=en.wikipedia.org",
                "width": 256,
                "height": 323
            }
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn public_search_uses_no_login_and_caches_the_selected_page() {
        let body = serde_json::json!({
            "query": {"pages": [
                {"pageid": 10, "title": "StarCraft", "index": 2},
                {
                    "pageid": 26809, "title": "StarCraft (video game)", "index": 1,
                    "original": {
                        "source": "https://upload.wikimedia.org/wikipedia/en/9/93/StarCraft_box_art.jpg",
                        "width": 256, "height": 323
                    }
                }
            ]}
        });
        let (base, worker) = super::super::tests::mock_api(200, "", &body.to_string());
        let service = ArtworkService::default();
        let mut client = WikipediaClient::new(&service).unwrap();
        client.base = base;
        let games = client.search("StarCraft & A/B").await.unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].source, ArtworkSource::Wikipedia);
        assert_eq!(games[0].id, 26809);
        let request = worker.join().unwrap();
        let target = request
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap();
        let url = url::Url::parse(&format!("http://localhost{target}")).unwrap();
        let query: std::collections::HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(
            query.get("gsrsearch").unwrap(),
            "StarCraft & A/B video game"
        );
        assert_eq!(query.get("pilicense").unwrap(), "any");
        assert!(!request.to_ascii_lowercase().contains("authorization:"));
        assert!(!request.to_ascii_lowercase().contains("cookie:"));
        assert!(!query.contains_key("key"));
        assert_eq!(
            client
                .details(26809)
                .await
                .unwrap()
                .images("grid", 0)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn covers_keep_source_ids_and_reject_other_hosts_or_unsupported_images() {
        let mut page = page_fixture();
        let images = page.images("grid", 0).unwrap();
        assert_eq!(images[0].id, 26809);
        assert_eq!(images[0].width, 256);
        assert_eq!(
            images[0].url,
            "https://upload.wikimedia.org/wikipedia/en/9/93/StarCraft_box_art.jpg"
        );
        assert!(page.images("hero", 0).unwrap().is_empty());
        assert!(page.images("grid", 1).unwrap().is_empty());
        for url in [
            "https://upload.wikimedia.org.attacker.test/cover.jpg",
            "https://attacker.test/cover.jpg",
            "https://upload.wikimedia.org/cover.svg",
            "http://upload.wikimedia.org/cover.jpg",
        ] {
            page.original.as_mut().unwrap().source = url.into();
            assert!(page.images("grid", 0).unwrap().is_empty(), "{url}");
        }
    }

    #[test]
    fn automatic_matching_does_not_confuse_original_games_with_editions_or_sequels() {
        let games = vec![
            super::super::ArtworkGame {
                id: 1,
                name: "StarCraft: Remastered".into(),
                source: ArtworkSource::Wikipedia,
            },
            super::super::ArtworkGame {
                id: 26809,
                name: "StarCraft (video game)".into(),
                source: ArtworkSource::Wikipedia,
            },
        ];
        assert_eq!(
            super::super::exact_match(&games, "starcraft").unwrap().id,
            26809
        );
        assert!(super::super::exact_match(&games, "StarCraft II").is_none());
        assert_eq!(matching_title("Doom (1993 video game)"), "Doom");
    }

    #[tokio::test]
    async fn wikipedia_rate_limits_do_not_block_steam_and_api_errors_are_not_empty_results() {
        for (status, body) in [(429, "{}"), (200, r#"{"error":{"code":"maxlag"}}"#)] {
            let (base, worker) = super::super::tests::mock_api(status, "Retry-After: 30\r\n", body);
            let service = ArtworkService::default();
            let mut client = WikipediaClient::new(&service).unwrap();
            client.base = base;
            assert!(client.search("StarCraft").await.is_err());
            worker.join().unwrap();
            assert!(service.retry_after.lock().unwrap().is_none());
            assert!(client
                .search("StarCraft")
                .await
                .unwrap_err()
                .contains("dočasne"));
        }
    }

    #[tokio::test]
    async fn source_selects_the_correct_catalogue_even_when_ids_overlap() {
        let service = ArtworkService::default();
        let page = page_fixture();
        service
            .wikipedia_pages
            .lock()
            .unwrap()
            .insert(26809, (Instant::now(), page));
        service.details.lock().unwrap().insert(
            26809,
            (Instant::now(), super::super::tests::details_fixture()),
        );
        let (wiki_name, wiki_images) =
            super::super::catalogue_images(&service, ArtworkSource::Wikipedia, 26809, "grid", 0)
                .await
                .unwrap();
        let (steam_name, steam_images) =
            super::super::catalogue_images(&service, ArtworkSource::Steam, 26809, "grid", 0)
                .await
                .unwrap();
        assert_eq!(wiki_name, "StarCraft (video game)");
        assert_eq!(steam_name, "Mafia: The Old Country");
        assert!(wiki_images[0].url.contains("upload.wikimedia.org"));
        assert!(steam_images[0].url.contains("steamstatic.com"));
    }

    #[tokio::test]
    #[ignore = "Live public catalogue fallback and actual image downloads without credentials"]
    async fn live_non_steam_covers_without_login() {
        let service = ArtworkService::default();
        let root = std::env::temp_dir().join(format!(
            "backup-sync-wikipedia-live-{}",
            uuid::Uuid::new_v4()
        ));
        for name in [
            "StarCraft",
            "StarCraft: Remastered",
            "Warcraft III: Reign of Chaos",
        ] {
            let (artwork, data) = super::super::automatic_artwork(&service, &root, name)
                .await
                .unwrap()
                .unwrap_or_else(|| panic!("No automatic cover for {name}"));
            assert_eq!(artwork.source, "wikipedia");
            assert!(data.starts_with("data:image/"));
            assert!(data.len() > 1000);
            // Re-open the saved catalogue reference with an empty service cache, as after restart.
            let fresh_service = ArtworkService::default();
            let (title, images) = super::super::catalogue_images(
                &fresh_service,
                ArtworkSource::Wikipedia,
                artwork.game_id,
                "grid",
                0,
            )
            .await
            .unwrap();
            assert_eq!(title, artwork.game_name);
            assert_eq!(images[0].url, artwork.url);
            assert_eq!(
                super::super::cached_image(&root, &artwork.url)
                    .await
                    .unwrap(),
                data
            );
            let saved = serde_json::to_string(&artwork).unwrap();
            let restored: crate::config::GameArtwork = serde_json::from_str(&saved).unwrap();
            assert_eq!(restored.source, "wikipedia");
            println!(
                "LIVE OK {name}: {} (Wikipedia #{}), {} bytes downloaded and cached",
                artwork.game_name,
                artwork.game_id,
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
