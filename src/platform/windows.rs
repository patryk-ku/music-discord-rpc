use crate::platform::{MediaInfo, NowPlayingResult};
use crate::utils::sanitize_name;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as WinSession,
    GlobalSystemMediaTransportControlsSessionManager as SessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};

pub fn get_startup_dir() -> Option<PathBuf> {
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

pub fn get_session_manager() -> Result<SessionManager, Box<dyn std::error::Error>> {
    let async_op = SessionManager::RequestAsync()?;
    let manager = async_op.join()?;
    Ok(manager)
}

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

pub fn session_media_props(
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

pub fn session_playback_status(
    session: &WinSession,
) -> Option<GlobalSystemMediaTransportControlsSessionPlaybackStatus> {
    session.GetPlaybackInfo().ok().and_then(|info| info.PlaybackStatus().ok())
}

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
