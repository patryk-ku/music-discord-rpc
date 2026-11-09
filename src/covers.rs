use pickledb::PickleDb;
use reqwest::blocking::Client;
use std::time::Duration;
use url_escape;

use crate::debug_log;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const TIMEOUT_SECONDS: u64 = 15;

/// Build a shared HTTP client with a 15-second timeout and application User-Agent.
fn build_client() -> Client {
    let user_agent = format!(
        "music-discord-rpc/{} (https://github.com/patryk-ku/music-discord-rpc)",
        VERSION
    );

    Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECONDS))
        .user_agent(user_agent)
        .build()
        .unwrap_or_else(|_| Client::new())
}

/// Helper function to normalize strings for comparison (lowercased, trimmed, trailing dots removed).
fn normalize(s: &str) -> String {
    s.trim()
        .trim_end_matches('.')
        .trim_end_matches('-')
        .trim()
        .to_lowercase()
}

/// Check if two strings match (exact, starts_with, or contains for strings of length >= 3).
fn fuzzy_match(candidate: &str, query: &str) -> bool {
    let c = normalize(candidate);
    let q = normalize(query);

    if c.is_empty() || q.is_empty() {
        return false;
    }

    if c == q {
        return true;
    }

    if q.len() >= 3 && (c.contains(&q) || q.contains(&c)) {
        return true;
    }

    false
}

/// Fetch album cover from Last.fm API.
pub fn get_cover_url_lastfm(
    client: &Client,
    artist: &str,
    album: &mut String,
    track_title: &str,
    lastfm_api_key: &str,
    debug_log: bool,
) -> Option<String> {
    if lastfm_api_key.trim().is_empty() {
        return None;
    }

    let is_unknown_album = album.as_str() == "Unknown Album";
    let request_url = if is_unknown_album {
        if track_title.trim().is_empty() {
            debug_log!(debug_log, "[last.fm] Missing album and track title.");
            return None;
        }
        format!(
            "https://ws.audioscrobbler.com/2.0/?method=track.getInfo&api_key={}&artist={}&track={}&autocorrect=1&format=json",
            lastfm_api_key,
            url_escape::encode_component(artist),
            url_escape::encode_component(track_title)
        )
    } else {
        format!(
            "https://ws.audioscrobbler.com/2.0/?method=album.getinfo&api_key={}&artist={}&album={}&autocorrect=0&format=json",
            lastfm_api_key,
            url_escape::encode_component(artist),
            url_escape::encode_component(album)
        )
    };

    let res = client.get(request_url).send().ok()?;
    let data: serde_json::Value = res.json().ok()?;

    let (url, resolved_album) = if is_unknown_album {
        let img = data["track"]["album"]["image"][3]["#text"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let alb = data["track"]["album"]["title"]
            .as_str()
            .map(|s| s.to_string());
        (img, alb)
    } else {
        let img = data["album"]["image"][3]["#text"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        (img, None)
    };

    if let Some(new_album) = resolved_album {
        if !new_album.trim().is_empty() && new_album != "Unknown Album" {
            debug_log!(debug_log, "[last.fm] resolved album name: {}", new_album);
            *album = new_album;
        }
    }

    if !url.is_empty() && url.len() > 5 {
        debug_log!(debug_log, "[last.fm] fetched image link: {}", url);
        return Some(url);
    }

    None
}

/// Fetch album cover and name from iTunes Search API.
pub fn get_cover_url_itunes(
    client: &Client,
    artist: &str,
    track_title: &str,
    album_out: &mut String,
    debug_log: bool,
) -> Option<String> {
    let term = format!("{} {}", artist.trim(), track_title.trim());
    let request_url = format!(
        "https://itunes.apple.com/search?term={}&entity=song&limit=5",
        url_escape::encode_component(&term)
    );

    let res = client.get(request_url).send().ok()?;
    let data: serde_json::Value = res.json().ok()?;
    let results = data["results"].as_array()?;

    for r in results {
        let r_art = r["artistName"].as_str().unwrap_or_default();
        let r_trk = r["trackName"].as_str().unwrap_or_default();

        let art_match = fuzzy_match(r_art, artist);
        let trk_match = fuzzy_match(r_trk, track_title);

        if art_match && trk_match {
            if let Some(col_name) = r["collectionName"].as_str() {
                if !col_name.is_empty() && album_out.as_str() == "Unknown Album" {
                    *album_out = col_name.to_string();
                    debug_log!(debug_log, "[itunes] resolved album name: {}", col_name);
                }
            }
            if let Some(art_url) = r["artworkUrl100"].as_str() {
                let high_res = art_url.replace("100x100bb", "512x512bb");
                debug_log!(debug_log, "[itunes] fetched image link: {}", high_res);
                return Some(high_res);
            }
        }
    }

    None
}

/// Fetch album cover and name from Deezer API.
pub fn get_cover_url_deezer(
    client: &Client,
    artist: &str,
    track_title: &str,
    album_out: &mut String,
    debug_log: bool,
) -> Option<String> {
    let term = format!("{} {}", artist.trim(), track_title.trim());
    let request_url = format!(
        "https://api.deezer.com/search?q={}",
        url_escape::encode_component(&term)
    );

    let res = client.get(request_url).send().ok()?;
    let data: serde_json::Value = res.json().ok()?;
    let results = data["data"].as_array()?;

    for r in results {
        let r_art = r["artist"]["name"].as_str().unwrap_or_default();
        let r_trk = r["title"].as_str().unwrap_or_default();

        let art_match = fuzzy_match(r_art, artist);
        let trk_match = fuzzy_match(r_trk, track_title);

        if art_match && trk_match {
            if let Some(col_name) = r["album"]["title"].as_str() {
                if !col_name.is_empty() && album_out.as_str() == "Unknown Album" {
                    *album_out = col_name.to_string();
                    debug_log!(debug_log, "[deezer] resolved album name: {}", col_name);
                }
            }
            if let Some(art_url) = r["album"]["cover_big"].as_str() {
                debug_log!(debug_log, "[deezer] fetched image link: {}", art_url);
                return Some(art_url.to_string());
            }
        }
    }

    None
}

/// Fetch album cover and name from MusicBrainz / Cover Art Archive API.
pub fn get_cover_url_musicbrainz(
    client: &Client,
    artist: &str,
    track_title: &str,
    album: &mut String,
    debug_log: bool,
) -> Option<String> {
    let is_unknown_album = album.as_str() == "Unknown Album";
    let request_url = if is_unknown_album {
        if track_title.trim().is_empty() {
            debug_log!(debug_log, "[musicbrainz] Missing album and track title.");
            return None;
        }
        format!(
            "https://musicbrainz.org/ws/2/recording/?query=recording:\"{}\" AND artist:\"{}\"&fmt=json&limit=5",
            url_escape::encode_component(track_title),
            url_escape::encode_component(artist)
        )
    } else {
        format!(
            "https://musicbrainz.org/ws/2/release/?query=artist:\"{}\" AND release:\"{}\"&fmt=json&limit=1",
            url_escape::encode_component(artist),
            url_escape::encode_component(album)
        )
    };

    let res = client.get(request_url).send().ok()?;
    let data: serde_json::Value = res.json().ok()?;

    let (mbid, resolved_album) = if is_unknown_album {
        let mut found_id = String::new();
        let mut found_title = None;

        if let Some(recordings) = data["recordings"].as_array() {
            for rec in recordings {
                let rec_title = rec["title"].as_str().unwrap_or_default();
                let artist_match = rec["artist-credit"].as_array().map(|credits| {
                    credits.iter().any(|c| {
                        let name = c["name"].as_str().unwrap_or_default();
                        fuzzy_match(name, artist)
                    })
                }).unwrap_or(false);

                let title_match = fuzzy_match(rec_title, track_title);

                if artist_match && title_match {
                    if let Some(releases) = rec["releases"].as_array() {
                        if let Some(rel) = releases.first() {
                            if let Some(id) = rel["id"].as_str() {
                                found_id = id.to_string();
                                found_title = rel["title"].as_str().map(|s| s.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }
        (found_id, found_title)
    } else {
        let id = data["releases"][0]["id"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        (id, None)
    };

    if let Some(new_album) = resolved_album {
        if !new_album.trim().is_empty() && new_album != "Unknown Album" {
            debug_log!(debug_log, "[musicbrainz] resolved album name: {}", new_album);
            *album = new_album;
        }
    }

    if !mbid.is_empty() && mbid.len() > 5 {
        let ca_url = format!("https://coverartarchive.org/release/{}/", mbid);
        if let Ok(ca_res) = client.get(ca_url).send() {
            if let Ok(ca_data) = ca_res.json::<serde_json::Value>() {
                if let Some(url) = ca_data["images"][0]["thumbnails"]["small"].as_str() {
                    debug_log!(debug_log, "[musicbrainz] fetched image link: {}", url);
                    return Some(url.to_string());
                }
            }
        }
    }

    None
}

/// Fetch user avatar from Last.fm API.
pub fn get_lastfm_avatar(username: &str, lastfm_api_key: &str, debug_log: bool) -> String {
    if username.trim().is_empty() || lastfm_api_key.trim().is_empty() {
        return String::new();
    }

    let client = build_client();
    let request_url = format!(
        "https://ws.audioscrobbler.com/2.0/?method=user.getinfo&api_key={}&user={}&format=json",
        lastfm_api_key,
        url_escape::encode_component(username)
    );

    if let Ok(res) = client.get(request_url).send() {
        if let Ok(data) = res.json::<serde_json::Value>() {
            if let Some(url) = data["user"]["image"][3]["#text"].as_str() {
                if !url.is_empty() && url.len() > 15 {
                    debug_log!(debug_log, "[last.fm] fetched avatar link: {}", url);
                    return url.to_string();
                }
            }
        }
    }

    String::new()
}

/// Main function to fetch album cover using prioritized cascade:
/// 1. Cache (if enabled)
/// 2. Last.fm (if API key present)
/// 3. iTunes
/// 4. Deezer
/// 5. MusicBrainz (if not disabled)
///
/// Automatically updates `album_id` and `media_info_album` if an unknown album is resolved,
/// and saves the result to `album_cache`.
pub fn fetch_cover(
    album_id: &mut String,
    media_info_album: &mut String,
    artist: &str,
    track_title: &str,
    lastfm_api_key: &str,
    cache_enabled: bool,
    album_cache: &mut PickleDb,
    disable_musicbrainz: bool,
    debug_log: bool,
) -> String {
    // 1. Check cache first before any network requests
    if cache_enabled && album_cache.exists(album_id) {
        if let Some(cached_url) = album_cache.get::<String>(album_id) {
            if !cached_url.is_empty() && cached_url.len() > 5 {
                debug_log!(debug_log, "[cache] loaded image url for: {}", album_id);
                return cached_url;
            }
        }
    }

    let client = build_client();
    let mut resolved_cover: Option<String> = None;

    // 2. Try Last.fm
    if !lastfm_api_key.trim().is_empty() {
        resolved_cover = get_cover_url_lastfm(
            &client,
            artist,
            media_info_album,
            track_title,
            lastfm_api_key,
            debug_log,
        );

        // Apple Music fallback: try stripping " - EP" and " - Single"
        if resolved_cover.is_none() {
            let album_name = media_info_album.trim();
            let suffix_len = if album_name.ends_with(" - EP") {
                Some(5)
            } else if album_name.ends_with(" - Single") {
                Some(9)
            } else {
                None
            };

            if let Some(len) = suffix_len {
                let stripped = album_name[..album_name.len() - len].trim().to_string();
                let mut stripped_copy = stripped.clone();
                debug_log!(
                    debug_log,
                    "Album cover not found, attempting without suffix: {} => {}",
                    album_name,
                    stripped
                );
                resolved_cover = get_cover_url_lastfm(
                    &client,
                    artist,
                    &mut stripped_copy,
                    track_title,
                    lastfm_api_key,
                    debug_log,
                );
                if resolved_cover.is_some() {
                    *media_info_album = stripped_copy;
                }
            }
        }
    }

    // 3. Try iTunes
    if resolved_cover.is_none() {
        resolved_cover = get_cover_url_itunes(
            &client,
            artist,
            track_title,
            media_info_album,
            debug_log,
        );
    }

    // 4. Try Deezer
    if resolved_cover.is_none() {
        resolved_cover = get_cover_url_deezer(
            &client,
            artist,
            track_title,
            media_info_album,
            debug_log,
        );
    }

    // 5. Try MusicBrainz
    if resolved_cover.is_none() && !disable_musicbrainz {
        resolved_cover = get_cover_url_musicbrainz(
            &client,
            artist,
            track_title,
            media_info_album,
            debug_log,
        );
    }

    // Save to cache and update album_id if found
    if let Some(cover_url) = resolved_cover {
        let new_id = format!("{} - {}", artist, media_info_album);
        if cache_enabled {
            let _ = album_cache.set(album_id, &cover_url);
            let _ = album_cache.set(&new_id, &cover_url);
            debug_log!(debug_log, "[cache] saved image url for: {}", new_id);
        }
        *album_id = new_id;
        return cover_url;
    }

    String::from("missing-cover")
}

// ==============================================================================
// NOTE: The tests below were fully generated by AI.
// ==============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};

    #[test]
    fn test_itunes_cover_lookup() {
        let client = build_client();
        let mut album = String::from("Unknown Album");
        let result = get_cover_url_itunes(&client, "Queen", "Bohemian Rhapsody", &mut album, true);

        assert!(result.is_some(), "iTunes should find Queen - Bohemian Rhapsody");
        let url = result.unwrap();
        assert!(url.starts_with("http"), "Cover URL must be valid HTTP(S)");
        assert!(url.contains("512x512bb"), "Artwork URL should be upgraded to 512x512");
        assert_ne!(album, "Unknown Album", "Album name should be resolved from iTunes");
    }

    #[test]
    fn test_deezer_cover_lookup() {
        let client = build_client();
        let mut album = String::from("Unknown Album");
        let result = get_cover_url_deezer(&client, "Daft Punk", "Get Lucky", &mut album, true);

        assert!(result.is_some(), "Deezer should find Daft Punk - Get Lucky");
        let url = result.unwrap();
        assert!(url.starts_with("http"), "Cover URL must be valid HTTP(S)");
    }

    #[test]
    fn test_musicbrainz_cover_lookup() {
        let client = build_client();
        let mut album = String::from("Nevermind");
        let result = get_cover_url_musicbrainz(&client, "Nirvana", "Smells Like Teen Spirit", &mut album, true);

        assert!(result.is_some(), "MusicBrainz should find Nirvana - Nevermind");
        let url = result.unwrap();
        assert!(url.starts_with("http"), "Cover URL must be valid HTTP(S)");
    }

    #[test]
    fn test_lastfm_cover_lookup_if_key_present() {
        let api_key = match option_env!("LASTFM_API_KEY") {
            Some(key) if !key.is_empty() => key.to_string(),
            _ => {
                if let Ok(content) = std::fs::read_to_string(".env") {
                    content
                        .lines()
                        .find(|l| l.starts_with("LASTFM_API_KEY="))
                        .and_then(|l| l.strip_prefix("LASTFM_API_KEY="))
                        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                        .unwrap_or_default()
                } else {
                    std::env::var("LASTFM_API_KEY").unwrap_or_default()
                }
            }
        };

        if api_key.trim().is_empty() {
            println!("Skipping Last.fm test: LASTFM_API_KEY is not set.");
            return;
        }

        let client = build_client();
        let mut album = String::from("A Night at the Opera");
        let result = get_cover_url_lastfm(&client, "Queen", &mut album, "Bohemian Rhapsody", &api_key, true);

        assert!(result.is_some(), "Last.fm should find Queen - A Night at the Opera");
        let url = result.unwrap();
        assert!(url.starts_with("http"), "Last.fm URL must be valid HTTP(S)");
    }

    #[test]
    fn test_fetch_cover_cache_flow() {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_music_discord_rpc_cache.db");
        let _ = std::fs::remove_file(&db_path);

        let mut db = PickleDb::new(
            &db_path,
            PickleDbDumpPolicy::NeverDump,
            SerializationMethod::Json,
        );

        let mut album_id = String::from("Test Artist - Test Album");
        let mut album = String::from("Test Album");

        // Manually prime the cache
        let dummy_url = "https://example.com/cached_cover.jpg".to_string();
        db.set(&album_id, &dummy_url).unwrap();

        // fetch_cover should immediately return cached URL without querying network
        let fetched = fetch_cover(
            &mut album_id,
            &mut album,
            "Test Artist",
            "Test Track",
            "",
            true,
            &mut db,
            true,
            true,
        );

        assert_eq!(fetched, dummy_url, "fetch_cover must return cached image url");
        let _ = std::fs::remove_file(&db_path);
    }
}
