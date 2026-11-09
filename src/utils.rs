use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use pickledb::PickleDb;
use reqwest;
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use serde_json;
use std::{env, fs, process};
use url_escape;

#[cfg(target_os = "linux")]
use mpris::{FindingError, Player, PlayerFinder};
#[cfg(target_os = "linux")]
use std::time::Duration;

// A common struct to hold song information, ensuring a consistent
// return type regardless of the platform.
#[derive(Debug)]
pub struct MediaInfo {
    pub title: String,
    pub artist: String,
    pub album_artist: String,
    pub album: String,
    pub is_playing: bool,
    pub duration: u64,
    pub position: u64,
    pub is_track_position: bool,
    pub art_url: String, // Link to cover art on the internet
    pub url: String,     // Link to the currently playing media on the internet
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    pub player_id: String,
}

// Use a Result to handle potential errors, like no media playing.
type NowPlayingResult = Result<MediaInfo, Box<dyn std::error::Error>>;

const VERSION: &str = env!("CARGO_PKG_VERSION");

// Use to print debug log if enabled with argument
#[macro_export]
macro_rules! debug_log {
    ($cond:expr, $($arg:tt)*) => {
        if $cond {
            println!("\x1b[34;1m[debug]\x1b[0m {}", format!($($arg)*));
        }
    };
}

#[cfg(target_os = "linux")]
fn is_systemd_present() {
    match process::Command::new("ps")
        .arg("-p")
        .arg("1")
        .arg("-o")
        .arg("comm=")
        .output()
    {
        Ok(output) => {
            let init_process = String::from_utf8_lossy(&output.stdout);
            if init_process.trim() == "systemd" {
                return;
            }
            println!("\x1b[33;1mINFO: Most likely systemd is not available on your system.\x1b[0m");
        }
        Err(_) => {
            println!(
                "\x1b[33;1mINFO: Could not detect if systemd is available on your system.\x1b[0m"
            )
        }
    }
    println!(
        "You can try using XDG Autostart instead to add the application to autostart without systemd."
    );
    println!("Use the \x1b[32;1m--xdg\x1b[0m flag with the subcommands like this: \x1b[32;1mmusic-discord-rpc enable --xdg\x1b[0m.");
}

#[cfg(target_os = "linux")]
pub fn enable_service() {
    match process::Command::new("systemctl")
        .arg("--user")
        .arg("daemon-reload")
        .status()
    {
        Ok(_) => println!("Reloaded user systemd services."),
        Err(_) => {
            println!("Failed to reload user systemd services.");
            is_systemd_present();
            process::exit(1);
        }
    }

    match process::Command::new("systemctl")
        .arg("--user")
        .arg("enable")
        .arg("--now")
        .arg("music-discord-rpc.service")
        .status()
    {
        Ok(_) => println!("Enabled and started user systemd service."),
        Err(_) => {
            println!("Failed to enable and start user systemd service.");
            is_systemd_present();
            process::exit(1);
        }
    }

    process::exit(0);
}

#[cfg(target_os = "linux")]
pub fn disable_service() {
    match process::Command::new("systemctl")
        .arg("--user")
        .arg("disable")
        .arg("--now")
        .arg("music-discord-rpc.service")
        .status()
    {
        Ok(_) => println!("Stopped and disabled user systemd service."),
        Err(_) => {
            println!("Failed to stop and disable user systemd service.");
            is_systemd_present();
            process::exit(1);
        }
    }

    process::exit(0);
}

#[cfg(target_os = "linux")]
pub fn restart_service() {
    match process::Command::new("systemctl")
        .arg("--user")
        .arg("restart")
        .arg("music-discord-rpc.service")
        .status()
    {
        Ok(_) => println!("Restarted user systemd service."),
        Err(_) => {
            println!("Failed to restart user systemd service.");
            process::exit(1);
        }
    }
    process::exit(0);
}

pub fn get_config_path() -> Option<std::path::PathBuf> {
    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
        Some(std::path::PathBuf::from(config_home))
    } else if let Some(appdata) = env::var_os("APPDATA") {
        Some(std::path::PathBuf::from(appdata))
    } else if let Some(home_dir) = env::var_os("HOME") {
        let mut path = std::path::PathBuf::from(home_dir);
        path.push(".config");
        Some(path)
    } else if let Some(userprofile) = env::var_os("USERPROFILE") {
        let mut path = std::path::PathBuf::from(userprofile);
        path.push(".config");
        Some(path)
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
pub fn add_xdg_autostart() {
    let mut desktopt_file_path = match get_config_path() {
        Some(path) => path,
        None => {
            println!("\x1b[31mWARNING: Failed to determine user config directory.\x1b[0m");
            process::exit(1);
        }
    };
    desktopt_file_path.push("autostart");
    desktopt_file_path.push("music-discord-rpc.desktop");

    let desktop_file_content = r#"[Desktop Entry]
Name=music-discord-rpc
Type=Application
Exec=music-discord-rpc
X-GNOME-Autostart-enabled=true
Hidden=false
StartupNotify=false
Terminal=false
"#;

    match fs::write(&desktopt_file_path, desktop_file_content) {
        Ok(_) => {
            println!(
                "Created file: \x1b[32;1m{}\x1b[0m ",
                desktopt_file_path.display()
            );
            println!("This RPC should now start automatically with your system if your DE/WM supports XDG Autostart.");
        }
        Err(_) => {
            println!("\x1b[31mERROR: Failed to create autostart .desktop file.\x1b[0m");
            process::exit(1);
        }
    }

    process::exit(0);
}

#[cfg(target_os = "linux")]
pub fn remove_xdg_autostart() {
    let mut desktopt_file_path = match get_config_path() {
        Some(path) => path,
        None => {
            println!("\x1b[31mWARNING: Failed to determine user config directory.\x1b[0m");
            process::exit(1);
        }
    };
    desktopt_file_path.push("autostart");
    desktopt_file_path.push("music-discord-rpc.desktop");

    match fs::remove_file(&desktopt_file_path) {
        Ok(_) => println!(
            "Removed file: \x1b[32;1m{}\x1b[0m ",
            desktopt_file_path.display()
        ),
        Err(_) => {
            println!("\x1b[31mERROR: Failed to remove autostart .desktop file.\x1b[0m");
            process::exit(1);
        }
    }

    process::exit(0);
}

#[cfg(target_os = "windows")]
use std::path::PathBuf;

#[cfg(target_os = "windows")]
fn get_startup_dir() -> Option<PathBuf> {
    env::var_os("APPDATA").map(|appdata| {
        let mut path = PathBuf::from(appdata);
        path.push("Microsoft");
        path.push("Windows");
        path.push("Start Menu");
        path.push("Programs");
        path.push("Startup");
        path
    })
}

#[cfg(target_os = "windows")]
pub fn enable_autostart() {
    let startup_dir = match get_startup_dir() {
        Some(dir) => dir,
        None => {
            println!("WARNING: Failed to determine user Startup directory.");
            process::exit(1);
        }
    };

    if !startup_dir.exists() {
        if let Err(_) = fs::create_dir_all(&startup_dir) {
            println!("ERROR: Failed to create Startup directory.");
            process::exit(1);
        }
    }

    let exe_path = match env::current_exe() {
        Ok(path) => path,
        Err(_) => {
            println!("ERROR: Failed to determine current executable path.");
            process::exit(1);
        }
    };

    let vbs_path = startup_dir.join("music-discord-rpc.vbs");
    let vbs_content = format!(
        "CreateObject(\"Wscript.Shell\").Run \"\"\"{}\"\"\", 0, False\r\n",
        exe_path.display()
    );

    match fs::write(&vbs_path, vbs_content) {
        Ok(_) => {
            println!(
                "Created autostart script in Startup folder: {}",
                vbs_path.display()
            );
            println!("music-discord-rpc will now start automatically in the background when you log in.");
        }
        Err(_) => {
            println!("ERROR: Failed to create autostart script in Startup folder.");
            process::exit(1);
        }
    }

    process::exit(0);
}

#[cfg(target_os = "windows")]
pub fn disable_autostart() {
    let startup_dir = match get_startup_dir() {
        Some(dir) => dir,
        None => {
            println!("WARNING: Failed to determine user Startup directory.");
            process::exit(1);
        }
    };

    let vbs_path = startup_dir.join("music-discord-rpc.vbs");

    if vbs_path.exists() {
        match fs::remove_file(&vbs_path) {
            Ok(_) => {
                println!("Removed autostart script from Startup folder.");
                println!("Autostart has been disabled.");
            }
            Err(_) => {
                println!("ERROR: Failed to remove autostart script from Startup folder.");
                process::exit(1);
            }
        }
    } else {
        println!("Autostart script does not exist.");
    }

    process::exit(0);
}

pub fn clear_activity(is_activity_set: &mut bool, client: &mut DiscordIpcClient) {
    if *is_activity_set {
        let is_activity_cleared = client.clear_activity().is_ok();

        if is_activity_cleared {
            *is_activity_set = false;
            return;
        }

        let is_reconnected = client.reconnect().is_ok();
        if !is_reconnected {
            return;
        }

        if client.clear_activity().is_ok() {
            *is_activity_set = false;
        }
    }
}

pub fn get_cover_url(
    album_id: &str,
    album: &mut String,
    mut _cover_url: String,
    cache_enabled: bool,
    album_cache: &mut PickleDb,
    artist: &str,
    lastfm_api_key: &str,
    track_title: &str,
) -> String {
    // Load from cache if enabled
    if cache_enabled {
        let cache_url = if album_cache.exists(&album_id) {
            match album_cache.get(&album_id) {
                Some(url) => url,
                None => String::new(),
            }
        } else {
            String::new()
        };

        if (!cache_url.is_empty()) && (cache_url.len() > 5) {
            return String::from(cache_url);
        }
    }

    let is_unknown_album = album.as_str() == "Unknown Album";
    let request_url = if is_unknown_album {
        if track_title.trim().is_empty() {
            println!("Missing album name or Unknown Album.");
            return String::from("missing-cover");
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

    let (url, resolved_album) = match reqwest::blocking::get(request_url) {
        Ok(res) => match res.json::<serde_json::Value>() {
            Ok(data) => {
                if is_unknown_album {
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
                }
            }
            Err(_) => (String::new(), None),
        },
        Err(_) => (String::new(), None),
    };

    if let Some(new_album) = resolved_album {
        if !new_album.trim().is_empty() {
            println!("[last.fm] resolved album name: {}", new_album);
            *album = new_album;
        }
    }

    if !url.is_empty() && (url.len() > 5) {
        println!("[last.fm] fetched image link: {}", url);

        // Save cover url to cache
        if cache_enabled {
            match album_cache.set(&album_id, &url) {
                Ok(_) => {
                    println!("[cache] saved image url for: {}.", album_id)
                }
                Err(_) => {
                    println!("[cache] error, unable to write to cache file.")
                }
            }
        }

        return url;
    }

    String::from("missing-cover")
}

pub fn get_cover_url_deezer(
    artist: &str,
    track_title: &str,
    album_out: &mut String,
) -> Option<String> {
    let term = format!("{} {}", artist.trim().trim_end_matches('.'), track_title.trim());
    let request_url = format!(
        "https://api.deezer.com/search?q={}",
        url_escape::encode_component(&term)
    );

    let client = Client::new();
    let res = client
        .get(request_url)
        .header(USER_AGENT, "curl")
        .send()
        .ok()?;

    let data: serde_json::Value = res.json().ok()?;
    let results = data["data"].as_array()?;

    let artist_norm = artist.trim().trim_end_matches('.').to_lowercase();
    let title_norm = track_title.trim().to_lowercase();

    for r in results {
        let r_art = r["artist"]["name"].as_str().unwrap_or_default().trim().trim_end_matches('.').to_lowercase();
        let r_trk = r["title"].as_str().unwrap_or_default().trim().to_lowercase();

        let art_match = r_art == artist_norm || r_art.contains(&artist_norm) || artist_norm.contains(&r_art);
        let trk_match = r_trk == title_norm || r_trk.contains(&title_norm) || title_norm.contains(&r_trk);

        if art_match && trk_match {
            if let Some(col_name) = r["album"]["title"].as_str() {
                if !col_name.is_empty() && album_out.as_str() == "Unknown Album" {
                    *album_out = col_name.to_string();
                    println!("[deezer] resolved album name: {}", col_name);
                }
            }
            if let Some(art_url) = r["album"]["cover_big"].as_str() {
                println!("[deezer] fetched image link: {}", art_url);
                return Some(art_url.to_string());
            }
        }
    }

    None
}

pub fn get_cover_url_itunes(
    artist: &str,
    track_title: &str,
    album_out: &mut String,
) -> Option<String> {
    let term = format!("{} {}", artist.trim().trim_end_matches('.'), track_title.trim());
    let request_url = format!(
        "https://itunes.apple.com/search?term={}&entity=song&limit=5",
        url_escape::encode_component(&term)
    );

    let client = Client::new();
    let res = client
        .get(request_url)
        .header(USER_AGENT, "curl")
        .send()
        .ok()?;

    let data: serde_json::Value = res.json().ok()?;
    let results = data["results"].as_array()?;

    let artist_norm = artist.trim().trim_end_matches('.').to_lowercase();
    let title_norm = track_title.trim().to_lowercase();

    for r in results {
        let r_art = r["artistName"].as_str().unwrap_or_default().trim().trim_end_matches('.').to_lowercase();
        let r_trk = r["trackName"].as_str().unwrap_or_default().trim().to_lowercase();

        let art_match = r_art == artist_norm || r_art.contains(&artist_norm) || artist_norm.contains(&r_art);
        let trk_match = r_trk == title_norm || r_trk.contains(&title_norm) || title_norm.contains(&r_trk);

        if art_match && trk_match {
            if let Some(col_name) = r["collectionName"].as_str() {
                if !col_name.is_empty() && album_out.as_str() == "Unknown Album" {
                    *album_out = col_name.to_string();
                    println!("[itunes] resolved album name: {}", col_name);
                }
            }
            if let Some(art_url) = r["artworkUrl100"].as_str() {
                let high_res = art_url.replace("100x100bb", "512x512bb");
                println!("[itunes] fetched image link: {}", high_res);
                return Some(high_res);
            }
        }
    }

    None
}

pub fn get_cover_url_musicbrainz(
    album_id: &str,
    album: &mut String,
    mut _cover_url: String,
    cache_enabled: bool,
    album_cache: &mut PickleDb,
    artist: &str,
    track_title: &str,
) -> String {
    // Load from cache if enabled
    if cache_enabled {
        let cache_url = if album_cache.exists(&album_id) {
            match album_cache.get(&album_id) {
                Some(url) => url,
                None => String::new(),
            }
        } else {
            String::new()
        };

        if (!cache_url.is_empty()) && (cache_url.len() > 5) {
            return String::from(cache_url);
        }
    }

    let user_agent = format!(
        "music-discord-rpc/{} (patryk.kurdziel@protonmail.com)",
        VERSION
    );

    let is_unknown_album = album.as_str() == "Unknown Album";
    let request_url = if is_unknown_album {
        if track_title.trim().is_empty() {
            println!("Missing album name or Unknown Album.");
            return String::from("missing-cover");
        }
        format!(
            "https://musicbrainz.org/ws/2/recording/?query=recording:\"{}\"ANDartist:\"{}\"&fmt=json&limit=5",
            url_escape::encode_component(track_title),
            url_escape::encode_component(artist)
        )
    } else {
        format!(
            "https://musicbrainz.org/ws/2/release/?query=artist:\"{}\"ANDrelease:\"{}\"&fmt=json&limit=1",
            url_escape::encode_component(artist),
            url_escape::encode_component(album)
        )
    };

    let client = Client::new();
    let (mbid, resolved_album) = match client
        .get(request_url)
        .header(USER_AGENT, &user_agent)
        .send()
    {
        Ok(res) => match res.json::<serde_json::Value>() {
            Ok(data) => {
                if is_unknown_album {
                    let mut found_id = String::new();
                    let mut found_title = None;
                    let artist_norm = artist.trim().trim_end_matches('.').to_lowercase();
                    let title_norm = track_title.trim().to_lowercase();

                    if let Some(recordings) = data["recordings"].as_array() {
                        for rec in recordings {
                            let rec_title = rec["title"].as_str().unwrap_or_default().trim().to_lowercase();
                            let artist_match = rec["artist-credit"].as_array().map(|credits| {
                                credits.iter().any(|c| {
                                    let name = c["name"].as_str().unwrap_or_default().trim().trim_end_matches('.').to_lowercase();
                                    name == artist_norm || name.contains(&artist_norm) || artist_norm.contains(&name)
                                })
                            }).unwrap_or(false);

                            let title_match = rec_title == title_norm
                                || rec_title.contains(&title_norm)
                                || title_norm.contains(&rec_title);

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
                }
            }
            Err(_) => (String::new(), None),
        },
        Err(_) => (String::new(), None),
    };

    if let Some(new_album) = resolved_album {
        if !new_album.trim().is_empty() {
            println!("[musicbrainz] resolved album name: {}", new_album);
            *album = new_album;
        }
    }

    if !mbid.is_empty() && (mbid.len() > 5) {
        let url: String = match client
            .get(format!("https://coverartarchive.org/release/{}/", mbid))
            .header(USER_AGENT, &user_agent)
            .send()
        {
            Ok(res) => match res.json::<serde_json::Value>() {
                Ok(data) => data["images"][0]["thumbnails"]["small"].to_string(),
                Err(_) => String::new(),
            },
            Err(_) => String::new(),
        };

        if !url.is_empty() && (url.len() > 5) {
            let mut clean_url = url;
            clean_url.pop();
            clean_url.remove(0);
            println!("[musicbrainz] fetched image link: {}", clean_url);

            // Save cover url to cache
            if cache_enabled {
                match album_cache.set(&album_id, &clean_url) {
                    Ok(_) => {
                        println!("[cache] saved image url for: {}.", album_id)
                    }
                    Err(_) => {
                        println!("[cache] error, unable to write to cache file.")
                    }
                }
            }

            return clean_url;
        }
    }

    String::from("missing-cover")
}

pub fn get_lastfm_avatar(username: &str, lastfm_api_key: &str) -> String {
    let request_url = format!(
        "https://ws.audioscrobbler.com/2.0/?method=user.getinfo&api_key={}&user={}&format=json",
        lastfm_api_key,
        url_escape::encode_component(username)
    );

    let mut url: String = match reqwest::blocking::get(request_url) {
        Ok(res) => match res.json::<serde_json::Value>() {
            Ok(data) => data["user"]["image"][3]["#text"].to_string(),
            Err(_) => String::new(),
        },
        Err(_) => String::new(),
    };

    if !url.is_empty() && (url.len() > 15) {
        url.pop();
        url.remove(0);
        println!("[last.fm] fetched avatar link: {}", url);
        return url;
    }

    return String::new();
}

pub fn sanitize_name(input: &str) -> String {
    input
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

#[cfg(target_os = "linux")]
pub fn get_currently_playing(player: &Player, debug_log: bool) -> NowPlayingResult {
    let metadata = match player.get_metadata() {
        Ok(metadata) => metadata,
        Err(err) => return Err(format!("Could not get metadata from player: {}", err).into()),
    };
    debug_log!(debug_log, "{:#?}", metadata);

    let playback_status = match player.get_playback_status() {
        Ok(status) => status,
        Err(err) => {
            return Err(format!("Could not get playback status from player: {}", err).into())
        }
    };

    let is_playing: bool = match playback_status {
        mpris::PlaybackStatus::Playing => true,
        mpris::PlaybackStatus::Paused => false,
        mpris::PlaybackStatus::Stopped => false,
    };
    debug_log!(debug_log, "playback_status: {:#?}", playback_status);

    // Parse metadata
    let title = metadata.title().unwrap_or("Unknown Title").to_string();
    let mut album = metadata.album_name().unwrap_or("Unknown Album").to_string();
    if album.is_empty() {
        album = "Unknown Album".to_string();
    }
    let artist = match metadata.artists() {
        Some(artists) => {
            if artists.is_empty() {
                "Unknown Artist".to_string()
            } else {
                artists[0].to_string()
            }
        }
        None => "Unknown Artist".to_string(),
    };
    let mut album_artist = match metadata.album_artists() {
        Some(artists) => {
            if artists.is_empty() {
                "Unknown Artist".to_string()
            } else {
                artists[0].to_string()
            }
        }
        None => "Unknown Artist".to_string(),
    };
    if album_artist.is_empty() || album_artist == "Unknown Artist" {
        album_artist = artist.clone();
    }

    // Get track duration if supported by player else return 0
    let duration = metadata.length().unwrap_or(Duration::new(0, 0)).as_secs();

    // Get track position if supported by player else return 0 secs
    let mut is_track_position: bool = false;
    let position = match player.get_position() {
        Ok(position) => {
            is_track_position = true;
            position.as_secs()
        }
        Err(_) => Duration::new(0, 0).as_secs(),
    };

    let art_url = match metadata.art_url() {
        Some(url) => url.to_string(),
        _ => String::new(),
    };

    let url = match metadata.url() {
        Some(url) => {
            let url_string = url.to_string();
            if url_string.starts_with("http://") || url_string.starts_with("https://") {
                url_string
            } else {
                String::new()
            }
        }
        _ => String::new(),
    };

    Ok(MediaInfo {
        title,
        artist,
        album_artist,
        album,
        is_playing,
        duration,
        position,
        is_track_position,
        art_url,
        url,
    })
}

#[cfg(target_os = "macos")]
pub fn get_currently_playing() -> NowPlayingResult {
    // PREREQUISITE: You must install this tool first!
    // ==> brew install media-control
    use std::process::Command;

    let output = Command::new("media-control").args(["get"]).output();

    match output {
        Ok(output) => {
            let result_str = String::from_utf8(output.stdout)?;
            if result_str.eq("null\n") {
                return Err("No media is currently playing. Waiting for any player...".into());
            }
            let json_result: serde_json::Value = serde_json::from_str(&result_str)?;

            let title = json_result["title"]
                .as_str()
                .unwrap_or("Unknown Title")
                .to_string();
            let artist = json_result["artist"]
                .as_str()
                .unwrap_or("Unknown Artist")
                .to_string();
            let album = json_result["album"]
                .as_str()
                .unwrap_or("Unknown Album")
                .to_string();
            let album_artist = artist.clone(); // Assuming album artist is the same as artist
            let is_playing = json_result["playing"].as_bool().unwrap_or(false);
            let duration = json_result["duration"].as_f64().unwrap_or(0.0) as u64;
            let position = json_result["elapsedTime"].as_f64().unwrap_or(0.0) as u64;
            let player_id = json_result["bundleIdentifier"]
                .as_str()
                .unwrap_or("Unknown Player")
                .to_string();
            let art_url = String::new(); // For now cant get artwork remote url like with mpris
            let is_track_position = true;
            let url = String::new();

            Ok(MediaInfo {
                title,
                artist,
                album_artist,
                album,
                is_playing,
                duration,
                position,
                is_track_position,
                art_url,
                url,
                player_id,
            })
        }
        Err(e) => {
            // This error usually means media-control is not installed or not in PATH.
            Err(format!(
                "Failed to execute 'media-control'. Is it installed? (brew install media-control) Error: {}",
                e
            ).into())
        }
    }
}

// Translate bundle id to app name for some players
#[cfg(target_os = "macos")]
pub fn app_name_from_bundle_id(bundle_id: &str) -> String {
    let name = match bundle_id {
        "com.apple.Music" => "Apple Music",
        "com.apple.iTunes" => "iTunes",
        "com.spotify.client" => "Spotify",
        "org.videolan.vlc" => "VLC media player",
        "com.colliderli.iina" => "IINA",
        "org.cogx.cog" => "Cog",
        "com.foobar2000.mac" => "foobar2000",
        "org.clementine-player.Clementine" => "Clementine",
        "com.jriver.MediaCenter" => "JRiver Media Center",
        "org.quodlibet.quodlibet" => "Quod Libet",
        "com.swinsian.Swinsian" => "Swinsian",
        "app.ytmdesktop.ytmdesktop" => "YouTube Music",
        "com.tidal.desktop" => "TIDAL",
        "com.deezer.desktop" => "Deezer",

        // Browsers
        "com.apple.Safari" => "Safari",
        "com.google.Chrome" => "Chrome",
        "org.chromium.Chromium" => "Chromium",
        "org.mozilla.firefox" => "Firefox",
        "com.microsoft.edgemac" => "Microsoft Edge",
        "com.brave.Browser" => "Brave",
        "com.opera.Opera" => "Opera",
        "com.vivaldi.Vivaldi" => "Vivaldi",

        // fallback
        other => other,
    };

    name.to_string()
}

#[cfg(target_os = "windows")]
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as WinSession,
    GlobalSystemMediaTransportControlsSessionManager as SessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};

#[cfg(target_os = "windows")]
pub fn get_session_manager() -> Result<SessionManager, Box<dyn std::error::Error>> {
    let async_op = SessionManager::RequestAsync()?;
    let manager = async_op.join()?;
    Ok(manager)
}

#[cfg(target_os = "windows")]
pub fn get_available_players() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let manager = get_session_manager()?;
    let mut list = Vec::new();

    if let Ok(sessions) = manager.GetSessions() {
        let count = sessions.Size().unwrap_or(0);
        for i in 0..count {
            if let Ok(session) = sessions.GetAt(i) {
                if let Ok(app_id) = session.SourceAppUserModelId() {
                    let id_str = app_id.to_string();
                    let name = app_name_from_app_id(&id_str);
                    if !list.contains(&name) {
                        list.push(name);
                    }
                }
            }
        }
    }

    Ok(list)
}

#[cfg(target_os = "windows")]
fn session_media_props(
    session: &WinSession,
) -> Option<(String, String, String, String)> {
    if let Ok(op) = session.TryGetMediaPropertiesAsync() {
        if let Ok(props) = op.join() {
            let title = props.Title().map(|s| s.to_string()).unwrap_or_default();
            let artist = props.Artist().map(|s| s.to_string()).unwrap_or_default();
            let album = props.AlbumTitle().map(|s| s.to_string()).unwrap_or_default();
            let subtitle = props.Subtitle().map(|s| s.to_string()).unwrap_or_default();

            // Ignore sessions without title or artist
            if !title.trim().is_empty() || !artist.trim().is_empty() {
                return Some((title, artist, album, subtitle));
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn session_playback_status(
    session: &WinSession,
) -> Option<GlobalSystemMediaTransportControlsSessionPlaybackStatus> {
    session.GetPlaybackInfo().ok().and_then(|info| info.PlaybackStatus().ok())
}

#[cfg(target_os = "windows")]
pub fn get_active_session(
    manager: &SessionManager,
    allowlist: &[String],
    allowlist_enabled: bool,
) -> Result<WinSession, Box<dyn std::error::Error>> {
    let current_session = manager.GetCurrentSession().ok();

    if allowlist_enabled && !allowlist.is_empty() {
        if let Ok(sessions) = manager.GetSessions() {
            let count = sessions.Size().unwrap_or(0);
            let mut matched_sessions = Vec::new();

            for i in 0..count {
                if let Ok(session) = sessions.GetAt(i) {
                    if let Ok(app_id) = session.SourceAppUserModelId() {
                        let id_str = app_id.to_string();
                        let clean_id = app_name_from_app_id(&id_str);
                        let is_allowed = allowlist.iter().any(|allowed| {
                            let a = allowed.to_lowercase();
                            let id_low = id_str.to_lowercase();
                            let clean_low = clean_id.to_lowercase();
                            let sanitized_clean = sanitize_name(&clean_id);
                            let sanitized_allowed = sanitize_name(allowed);
                            id_low == a
                                || clean_low == a
                                || id_low.contains(&a)
                                || clean_low.contains(&a)
                                || sanitized_clean == sanitized_allowed
                        });

                        if is_allowed && session_media_props(&session).is_some() {
                            matched_sessions.push(session);
                        }
                    }
                }
            }

            // Prioritize focused session if it is on allowlist
            if let Some(ref curr) = current_session {
                if let Ok(curr_id) = curr.SourceAppUserModelId() {
                    let curr_matches = matched_sessions.iter().any(|s| {
                        s.SourceAppUserModelId().map(|id| id == curr_id).unwrap_or(false)
                    });
                    if curr_matches {
                        if let Some(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing) = session_playback_status(curr) {
                            return Ok(curr.clone());
                        }
                    }
                }
            }

            for session in &matched_sessions {
                if let Some(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing) = session_playback_status(session) {
                    return Ok(session.clone());
                }
            }

            if let Some(session) = matched_sessions.into_iter().next() {
                return Ok(session);
            }
        }
        return Err("Could not find any active player from your allowlist. Waiting for any player from your allowlist...".into());
    }

    // Check focused session first
    if let Some(ref curr) = current_session {
        if session_media_props(curr).is_some() {
            return Ok(curr.clone());
        }
    }

    // Fallback: search other active sessions
    if let Ok(sessions) = manager.GetSessions() {
        let count = sessions.Size().unwrap_or(0);
        let mut first_valid_session = None;

        for i in 0..count {
            if let Ok(session) = sessions.GetAt(i) {
                if session_media_props(&session).is_some() {
                    if let Some(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing) = session_playback_status(&session) {
                        return Ok(session);
                    }
                    if first_valid_session.is_none() {
                        first_valid_session = Some(session);
                    }
                }
            }
        }

        if let Some(curr) = current_session {
            if session_media_props(&curr).is_some() {
                return Ok(curr);
            }
        }

        if let Some(session) = first_valid_session {
            return Ok(session);
        }
    }

    Err("No media is currently playing. Waiting for any player...".into())
}

#[cfg(target_os = "windows")]
pub fn get_currently_playing(
    allowlist: &[String],
    allowlist_enabled: bool,
) -> NowPlayingResult {
    let manager = get_session_manager()?;
    let session = get_active_session(&manager, allowlist, allowlist_enabled)?;

    let app_id = session
        .SourceAppUserModelId()
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "Unknown Player".to_string());

    let player_id = app_name_from_app_id(&app_id);

    let (title_raw, artist_raw, album_raw, subtitle_raw) = match session_media_props(&session) {
        Some(props) => props,
        None => return Err("No track information available. Waiting for any player...".into()),
    };

    let title = if title_raw.trim().is_empty() {
        "Unknown Title".to_string()
    } else {
        title_raw.trim().to_string()
    };

    let artist = if artist_raw.trim().is_empty() {
        "Unknown Artist".to_string()
    } else {
        artist_raw.trim().to_string()
    };

    let album_artist = artist.clone();

    // Fallback to subtitle if album title is empty
    let album = if !album_raw.trim().is_empty() {
        album_raw.trim().to_string()
    } else if !subtitle_raw.trim().is_empty() && subtitle_raw.trim().to_lowercase() != artist.to_lowercase() {
        subtitle_raw.trim().to_string()
    } else {
        "Unknown Album".to_string()
    };

    let is_playing = match session_playback_status(&session) {
        Some(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing) => true,
        _ => false,
    };

    let mut duration = 0u64;
    let mut position = 0u64;
    let mut is_track_position = false;

    if let Ok(timeline) = session.GetTimelineProperties() {
        if let Ok(end_time) = timeline.EndTime() {
            let d = end_time.Duration / 10_000_000;
            if d > 0 {
                duration = d as u64;
                is_track_position = true;
            }
        }
        if let Ok(pos) = timeline.Position() {
            let p = pos.Duration / 10_000_000;
            if p >= 0 {
                position = p as u64;
            }
        }
    }

    Ok(MediaInfo {
        title,
        artist,
        album_artist,
        album,
        is_playing,
        duration,
        position,
        is_track_position,
        art_url: String::new(),
        url: String::new(),
        player_id,
    })
}

#[cfg(target_os = "windows")]
pub fn app_name_from_app_id(app_id: &str) -> String {
    let lower = app_id.to_lowercase();
    if lower.contains("spotify") {
        "Spotify".to_string()
    } else if lower.contains("youtube-music")
        || lower.contains("ytmdesktop")
        || lower.contains("th-ch")
        || lower.contains("youtubemusic")
    {
        "YouTube Music".to_string()
    } else if lower.contains("msedge") {
        "Microsoft Edge".to_string()
    } else if lower.contains("chrome") {
        "Google Chrome".to_string()
    } else if lower.contains("firefox") {
        "Firefox".to_string()
    } else if lower.contains("brave") {
        "Brave".to_string()
    } else if lower.contains("opera") {
        "Opera".to_string()
    } else if lower.contains("vivaldi") {
        "Vivaldi".to_string()
    } else if lower.contains("vlc") {
        "VLC media player".to_string()
    } else if lower.contains("foobar2000") {
        "foobar2000".to_string()
    } else if lower.contains("aimp") {
        "AIMP".to_string()
    } else if lower.contains("itunes") {
        "iTunes".to_string()
    } else if lower.contains("applemusic") || lower.contains("apple music") {
        "Apple Music".to_string()
    } else if lower.contains("cider") {
        "Cider".to_string()
    } else if lower.contains("yandex") {
        "Yandex Music".to_string()
    } else if lower.contains("music.ui") || lower.contains("zune") {
        "Media Player".to_string()
    } else if lower.contains("tidal") {
        "TIDAL".to_string()
    } else if lower.contains("deezer") {
        "Deezer".to_string()
    } else {
        if let Some(stripped) = app_id.strip_suffix(".exe") {
            stripped.to_string()
        } else {
            app_id.to_string()
        }
    }
}

// Functions used to trim values to 256 bytes (Discord RPC limitation)
pub fn trim_to_max_bytes(mut input: String, max: usize) -> String {
    if input.len() <= max {
        return input;
    }

    let mut end = max;

    while !input.is_char_boundary(end) {
        end -= 1;
    }

    input.truncate(end);
    input
}

const MAX_URL_LEN: usize = 256;

fn encoded_len_of_byte(b: u8) -> usize {
    match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => 1,
        _ => 3,
    }
}

pub fn build_trimmed_url(prefix: &str, component: &str) -> String {
    let encoded = url_escape::encode_component(component);
    let full_url = format!("{prefix}{encoded}");

    if full_url.len() <= MAX_URL_LEN {
        return full_url;
    }

    let prefix_len = prefix.len();

    // if prefix_len >= MAX_URL_LEN {
    //     return prefix[..MAX_URL_LEN].to_string();
    // }

    let remaining = MAX_URL_LEN - prefix_len;

    let mut encoded_len = 0;
    let mut end = 0;

    for (idx, ch) in component.char_indices() {
        let mut char_encoded_len = 0;

        for &b in ch.encode_utf8(&mut [0; 4]).as_bytes() {
            char_encoded_len += encoded_len_of_byte(b);
        }

        if encoded_len + char_encoded_len > remaining {
            break;
        }

        encoded_len += char_encoded_len;
        end = idx + ch.len_utf8();
    }

    let trimmed = &component[..end];
    let encoded = url_escape::encode_component(trimmed);

    format!("{prefix}{encoded}")
}

#[cfg(target_os = "linux")]
pub fn allowlist_player_finder(
    player: &PlayerFinder,
    allowlist: &Vec<String>,
    debug_log: bool,
) -> Result<Player, FindingError> {
    let mut allowlist_finder = Err(mpris::FindingError::NoPlayerFound);

    // Find all players and sort them by allow list order then return the first one
    if let Ok(all_players) = player.find_all() {
        let mut found_players: Vec<_> = all_players
            .into_iter()
            .filter(|p| {
                !p.bus_name().eq("org.mpris.MediaPlayer2.playerctld")
                    && allowlist.contains(&p.identity().to_string())
            })
            .collect();

        if !found_players.is_empty() {
            debug_log!(debug_log, "Allowlist sorting:");
            // Allowlist sorting priority: playing > has metadata > allowlist order
            found_players.sort_by_key(|p| {
                // Check if player is currently playing
                let is_playing = p
                    .get_playback_status()
                    .unwrap_or(mpris::PlaybackStatus::Stopped)
                    == mpris::PlaybackStatus::Playing;

                // Check if metadata is complete (artist, title, and album)
                let mut is_metadata_complete = false;
                if let Ok(m) = p.get_metadata() {
                    let has_artist = match m.artists() {
                        Some(a) => !a.is_empty(),
                        None => false,
                    };
                    let has_title = match m.title() {
                        Some(t) => !t.is_empty(),
                        None => false,
                    };
                    let has_album = match m.album_name() {
                        Some(a) => !a.is_empty(),
                        None => false,
                    };
                    is_metadata_complete = has_artist && has_title && has_album;
                }

                debug_log!(
                    debug_log,
                    " - {}, playing: {}, metadata: {}",
                    p.identity(),
                    is_playing,
                    is_metadata_complete
                );

                (
                    !is_playing,
                    !is_metadata_complete,
                    allowlist
                        .iter()
                        .position(|allowlisted_name| allowlisted_name == p.identity())
                        .unwrap_or(usize::MAX),
                )
            });

            debug_log!(
                debug_log,
                "Selected player: {}",
                found_players[0].identity()
            );
            allowlist_finder = Ok(found_players.remove(0));
        }
    }
    allowlist_finder
}
