# Game artwork

Artwork is separate from cloud backup authentication. The app queries public catalogues by game title and caches selected images locally.

## Providers

| Source | Requests | Available images |
| --- | --- | --- |
| Steam | Public `storesearch/` and `appdetails` endpoints under `https://store.steampowered.com/api/` | Header, background, and screenshots returned for the selected app |
| Wikipedia | Public MediaWiki Action API at `https://en.wikipedia.org/w/api.php` | The article's main image through `pageimages` |
| Legacy SteamGridDB selections | Existing saved image URLs | Previously selected images remain loadable from the permitted CDN |

Current search and image selection use `steam` or `wikipedia`. They send no artwork-service API key, login cookie, or authorization header. Cloud provider accounts are configured separately.

Implementation references are [artwork.rs](../src-tauri/src/artwork.rs), [wikipedia.rs](../src-tauri/src/artwork/wikipedia.rs), and [GameArtworkPicker.tsx](../src/components/GameArtworkPicker.tsx).

## Automatic lookup

A game card requests automatic artwork only when the game has no saved selection and its name has not already been checked.

Steam is tried first, followed by Wikipedia when a usable matching image is unavailable. Matching normalizes letter case and punctuation. Wikipedia's video-game disambiguation suffix can be removed during matching. Ambiguous matches require manual selection.

The app tries cover images before screenshots and saves the first image it successfully downloads. The selected source, catalogue game ID, image ID, URL, credit, and manual-selection flag are persisted with the game.

Automatic lookup records a checked-name marker. The current internal marker retains the `steam:` prefix for compatibility even though lookup can use both catalogues.

## Manual selection

1. Open a game card's settings.
2. Select **Vybrať obrázok**.
3. Search for a title and choose the correct game or edition.
4. Click an image to save it.

Opening game settings alone does not load a gallery. Opening the image picker searches both sources concurrently. Results include their source, and IDs are interpreted within that source.

If one source fails but the other returns games, the picker displays those results and a warning. An empty combined result with a provider error is reported as an error so it is not mistaken for a confirmed missing game.

Steam separates `grid` cover/background images from `hero` screenshots, with zero-based pages of up to 50 images. Wikipedia provides the main article image on `grid` page zero. Its picker hides screenshot and pagination controls.

Clearing the image restores the default card background and records the current name as checked, preventing immediate automatic reselection. Search manually to choose another image.

## Caching and network bounds

- Catalogue details are cached in memory for up to 15 minutes, separately per source, with a bounded number of entries.
- Each source has its own temporary retry deadline after request failures or rate limits.
- Artwork requests use an eight-second connection timeout and a 25-second overall timeout.
- Catalogue responses are limited to 4 MiB. Image downloads are limited to 8 MiB.
- Image signatures must identify PNG, JPEG, or WebP data.
- Image URLs require HTTPS, permitted hosts, no embedded credentials, and no explicit nondefault port. Redirects are disabled for the artwork HTTP client.

Allowed media hosts cover Steam CDN subdomains under `.steamstatic.com`, `steamcdn-a.akamaihd.net`, `upload.wikimedia.org`, and the legacy `cdn2.steamgriddb.com` host. Wikipedia URLs are additionally restricted to its image host and supported file extensions.

Images are cached as data URLs under `state/artwork/<SHA-256-of-URL>.data-url` in the app configuration directory. Cached images are validated before reuse and can load offline. Downloads of uncached images still require access to the source.

Wikipedia can return small cover images, unrelated search hits, or no suitable article image. The user selects the edition when automatic matching cannot decide. The source label identifies where the image came from and does not transfer image ownership.

## Verification

The normal backend suite tests query encoding, credential-free requests, matching, source-specific IDs, caching, URL validation, and rate-limit handling.

Run the ignored live tests explicitly when verifying the actual catalogue and image-download path.

```powershell
cargo test --manifest-path src-tauri/Cargo.toml live_public_steam_artwork_without_login -- --ignored --nocapture
cargo test --manifest-path src-tauri/Cargo.toml live_non_steam_covers_without_login -- --ignored --nocapture
```

These tests access public services and create temporary image caches. Their success validates catalogue access and image downloads at that time. They do not test authenticated backup providers.
