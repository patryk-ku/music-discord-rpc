use crate::platform::{MediaInfo, NowPlayingResult};
use serde_json;
use std::process::Command;

pub fn get_currently_playing() -> NowPlayingResult {
    // PREREQUISITE: You must install this tool first!
    // ==> brew install media-control
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
