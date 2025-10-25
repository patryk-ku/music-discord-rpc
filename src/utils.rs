use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use pickledb::PickleDb;
use reqwest;
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use serde_json;
use std::env;

#[cfg(target_os = "linux")]
use mpris::Player;
#[cfg(target_os = "linux")]
use std::time::Duration;
#[cfg(target_os = "linux")]
use std::{fs, process};

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
    #[cfg(target_os = "macos")]
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
    } else if let Some(home_dir) = env::var_os("HOME") {
        let mut path = std::path::PathBuf::from(home_dir);
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
    album: &str,
    mut _cover_url: String,
    cache_enabled: bool,
    album_cache: &mut PickleDb,
    artist: &str,
    lastfm_api_key: &str,
) -> String {
    // If no album or Unknown Album
    if album.eq("Unknown Album") {
        println!("Missing album name or Unknown Album.");

        return String::from("missing-cover");
    }

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

    let request_url = format!(
    	"https://ws.audioscrobbler.com/2.0/?method=album.getinfo&api_key={}&artist={}&album={}&autocorrect=0&format=json",
     	lastfm_api_key,
     	url_escape::encode_component(artist),
     	url_escape::encode_component(album)
    );

    let mut url: String = match reqwest::blocking::get(request_url) {
        Ok(res) => match res.json::<serde_json::Value>() {
            Ok(data) => data["album"]["image"][3]["#text"].to_string(),
            Err(_) => String::new(),
        },
        Err(_) => String::new(),
    };

    if !url.is_empty() && (url.len() > 5) {
        url.pop();
        url.remove(0);
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

    return String::from("missing-cover");
}

pub fn get_cover_url_musicbrainz(
    album_id: &str,
    album: &str,
    mut _cover_url: String,
    cache_enabled: bool,
    album_cache: &mut PickleDb,
    artist: &str,
) -> String {
    // If no album or Unknown Album
    if album.eq("Unknown Album") {
        println!("Missing album name or Unknown Album.");

        return String::from("missing-cover");
    }

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

    let request_url = format!(
    	"https://musicbrainz.org/ws/2/release/?query=artist:\"{}\"ANDrelease:\"{}\"&fmt=json&limit=1",
    	url_escape::encode_component(artist),
     	url_escape::encode_component(album)
    );

    let client = Client::new();
    let mut mbid: String = match client
        .get(request_url)
        .header(USER_AGENT, &user_agent)
        .send()
    {
        Ok(res) => match res.json::<serde_json::Value>() {
            Ok(data) => data["releases"][0]["id"].to_string(),
            Err(_) => String::new(),
        },
        Err(_) => String::new(),
    };

    if !mbid.is_empty() && (mbid.len() > 5) {
        mbid.pop();
        mbid.remove(0);
    }

    let mut url: String = match client
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
        url.pop();
        url.remove(0);
        println!("[musicbrainz] fetched image link: {}", url);

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

    return String::from("missing-cover");
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
