use crate::debug_log;
use crate::platform::{MediaInfo, NowPlayingResult};
use crate::utils::get_config_path;
use mpris::{FindingError, Player, PlayerFinder};
use std::fs;
use std::process;
use std::time::Duration;

pub fn is_systemd_present() {
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
