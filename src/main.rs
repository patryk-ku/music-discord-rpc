use discord_rich_presence::activity::StatusDisplayType;
use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};
use url_escape;

#[cfg(target_os = "linux")]
use mpris::PlayerFinder;

use std::env;
use std::fs;
use std::ops::Sub;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, SystemTime};

mod settings;
mod utils;

// Load api key from .env file durning compilation
const LASTFM_API_KEY: &'static str = match option_env!("LASTFM_API_KEY") {
    Some(key) => key,
    None => "",
};

fn matches_allowlist(player_name: &str, pattern: &str) -> bool {
    pattern.strip_suffix('*')
        .map_or_else(|| player_name == pattern, |prefix| player_name.starts_with(prefix))
}

#[cfg(target_os = "linux")]
fn is_player_allowlisted(player: &mpris::Player, allowlist: &[String]) -> bool {
    let identity = player.identity();
    let bus_name = player.bus_name();
    allowlist.iter().any(|pattern| {
        matches_allowlist(identity, pattern) || matches_allowlist(bus_name, pattern)
    })
}

#[cfg(target_os = "linux")]
fn get_playback_priority(player: &mpris::Player) -> u8 {
    match player.get_playback_status() {
        Ok(mpris::PlaybackStatus::Playing) => 0,
        Ok(mpris::PlaybackStatus::Paused) => 1,
        _ => 2,
    }
}

#[cfg(target_os = "linux")]
fn has_valid_metadata(meta: &mpris::Metadata) -> bool {
    let has_title = meta.title().is_some();
    let has_artist = meta.artists()
        .map(|a| a.first().map_or(false, |s| !s.is_empty()))
        .unwrap_or(false);
    has_title && has_artist
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set home path, If $HOME is not set, do not write or read anything from the user's disk
    let (home_exists, home_dir) = match env::var("HOME") {
        Ok(val) => (true, PathBuf::from(val)),
        Err(_) => (false, PathBuf::from("/")),
    };

    let settings = settings::load_settings();

    debug_log!(settings.debug_log, "Settings: {:#?}", settings);
    debug_log!(settings.debug_log, "home_exists: {}", home_exists);
    debug_log!(settings.debug_log, "home_dir: {}", home_dir.display());

    // Exec subcommands
    #[cfg(target_os = "linux")]
    match settings.suboptions.command {
        Some(settings::Commands::Enable { xdg }) => {
            if xdg {
                utils::add_xdg_autostart()
            } else {
                utils::enable_service()
            }
        }
        Some(settings::Commands::Disable { xdg }) => {
            if xdg {
                utils::remove_xdg_autostart()
            } else {
                utils::disable_service()
            }
        }
        Some(settings::Commands::Restart {}) => utils::restart_service(),
        None => {}
    }
    #[cfg(target_os = "macos")]
    match settings.suboptions.command {
        Some(_) => {
            println!("Subcommands to manage the daemon are not available on macOS.");
            println!(
                "Check: https://github.com/patryk-ku/music-discord-rpc?tab=readme-ov-file#macos-3"
            );
            std::process::exit(0);
        }
        None => {}
    }

    // User settings

    // Use api key provided by user
    let lastfm_api_key = settings.lastfm_api_key.unwrap_or(LASTFM_API_KEY.into());
    if lastfm_api_key.is_empty() {
        println!("\x1b[31mWARNING: Last.fm API key is not set. Album covers from Last.fm will not be available.\x1b[0m");
    }

    // Main loop interval
    let mut interval = settings.interval.unwrap_or(10);
    if interval < 5 {
        interval = 5
    }
    debug_log!(settings.debug_log, "interval: {}", interval);

    // Nicknames for buttons
    let lastfm_name = settings.lastfm_name.unwrap_or_default();
    let listenbrainz_name = settings.listenbrainz_name.unwrap_or_default();

    // "Listening to ..."
    let rpc_name = settings.rpc_name.unwrap_or(String::from("artist"));

    // Icon displayed next to the album cover
    let small_image = settings.small_image.unwrap_or(String::from("playPause"));
    let mut lastfm_avatar = String::new();
    if small_image == "lastfmAvatar" && !lastfm_name.is_empty() {
        lastfm_avatar = utils::get_lastfm_avatar(&lastfm_name, &lastfm_api_key);
        debug_log!(settings.debug_log, "lastfm_avatar: {}", lastfm_avatar);
    }
    let lastfm_icon_text = if !lastfm_name.is_empty() {
        lastfm_name.to_string() + " on Last.fm"
    } else {
        String::new()
    };

    // Force player id and name
    let force_player_name = settings.force_player_name.unwrap_or_default();
    let force_player_id = settings.force_player_id.unwrap_or_default();

    // Enable/disable use of cache
    let mut cache_enabled: bool = !settings.disable_cache;
    if !home_exists {
        cache_enabled = false;
    }

    // Allowlist of music players
    let allowlist_enabled: bool = match settings.allowlist.len() {
        0 => false,
        _ => true,
    };

    // Vars for activity update detection
    let mut last_title: String = String::new();
    let mut last_album: String = String::new();
    let mut last_artist: String = String::new();
    let mut last_album_artist: String = String::new();
    let mut last_album_id: String = String::new();
    let mut last_track_position: u64 = 0;
    let mut last_is_playing: bool = false;

    let mut _cover_url: String = "".to_string();
    let mut is_first_time_audio: bool = true;
    let mut is_first_time_video: bool = true;
    let mut is_interrupted: bool = false;
    let mut is_activity_set: bool = false;

    // Preventing stdout spam while waiting for player or discord
    #[cfg(target_os = "linux")]
    let mut dbus_notif: bool = false;
    let mut player_notif: u8 = 0;
    let mut discord_notif: bool = false;

    let mut client_audio = DiscordIpcClient::new("1129859263741837373");
    let mut client_video = DiscordIpcClient::new("1356756023813210293");
    let mut client: &mut DiscordIpcClient = &mut client_audio;

    // Set cache path
    let cache_dir = match env::var("XDG_CACHE_HOME") {
        Ok(xgd_cache_home) => PathBuf::from(xgd_cache_home).join("music-discord-rpc"),
        Err(_) => home_dir.join(".cache/music-discord-rpc"),
    };

    if cache_enabled {
        debug_log!(
            settings.debug_log,
            "Cache location: {}",
            &cache_dir.display()
        );
        if let Err(err) = fs::create_dir_all(&cache_dir) {
            println!("Could not create cache directory: {}", err);
        }
    }

    // Cache file
    let db_path = cache_dir.join("album_cache.db");
    let mut album_cache = match PickleDb::load(
        &db_path,
        PickleDbDumpPolicy::AutoDump,
        SerializationMethod::Json,
    ) {
        Ok(db) => {
            if cache_enabled {
                println!("Cache loaded from file: {}", &db_path.display());
            }
            db
        }
        Err(_) => {
            if cache_enabled {
                println!("Generated new cache file: {}", &db_path.display());
            }
            PickleDb::new(
                &db_path,
                PickleDbDumpPolicy::AutoDump,
                SerializationMethod::Json,
            )
        }
    };

    loop {
        debug_log!(
            settings.debug_log,
            "───────────────────────────────Loop─1───────────────────────────────────"
        );

        // On Linux try to connect to MPRIS
        #[cfg(target_os = "linux")]
        let player_finder = match PlayerFinder::new() {
            Ok(player) => {
                dbus_notif = false;
                player
            }
            Err(err) => {
                if !dbus_notif {
                    println!("Could not connect to D-Bus: {}", err);
                    dbus_notif = true;
                }
                sleep(Duration::from_secs(interval));
                continue;
            }
        };

        // List available players and exit
        if settings.list_players {
            #[cfg(target_os = "linux")]
            match player_finder.find_all() {
                Ok(player_list) => {
                    if player_list.is_empty() {
                        println!("Could not find any player with MPRIS support.");
                    } else {
                        println!("");
                        println!("────────────────────────────────────────────────────");
                        println!("List of available music players with MPRIS support:");
                        for music_player in &player_list {
                            if music_player.bus_name() != "org.mpris.MediaPlayer2.playerctld" {
                                println!(" * {} ({})", music_player.identity(), music_player.bus_name());
                            }
                        }

                        // Find first non-playerctld player for usage examples
                        if let Some(example_player) = player_list.iter()
                            .find(|p| p.bus_name() != "org.mpris.MediaPlayer2.playerctld")
                        {
                            println!("");
                            println!("Use the name or bus name to choose from which source the script should take data for the Discord status.");
                            println!("Usage instructions:");
                            println!("");
                            println!(r#" music-discord-rpc -a "{}""#, example_player.identity());
                            println!(r#" music-discord-rpc -a "{}""#, example_player.bus_name());
                            println!("");
                            println!("You can use the -a argument multiple times to add more than one player to the allowlist:");
                            println!("");
                            println!(
                                r#" music-discord-rpc -a "{}" -a "Second Player" -a "Any other player""#,
                                example_player.identity()
                            );
                        }
                    }
                }
                Err(_) => {
                    println!("Could not find any player with MPRIS support.");
                }
            };

            #[cfg(target_os = "macos")]
            {
                println!("");
                println!("Displaying the list of players is not supported on macOS.");
                println!(
                    "However, it's possible to show the name of the currently detected player."
                );

                match utils::get_currently_playing() {
                    Ok(player) => {
                        println!("Player name: {}", player.player_id);
                        println!("");
                        println!(
                            "You can use this name together with the -a flag to add this player to the allowlist:"
                        );
                        println!(r#" music-discord-rpc -a "{}""#, player.player_id);
                        println!("");
                        println!("You can use the -a argument multiple times to add more than one player to the allowlist:");
                        println!(
                            r#" music-discord-rpc -a "{}" -a "Second Player" -a "Any other player""#,
                            player.player_id
                        );
                    }
                    Err(_) => {
                        println!("No player detected.");
                    }
                };
            }

            return Ok(());
        }

        // Find active player (and filter them by name if enabled)
        #[cfg(target_os = "linux")]
        let selected_player = if allowlist_enabled {
            let mut allowlist_finder = Err(mpris::FindingError::NoPlayerFound);

            // Find all players and select by priority
            if let Ok(all_players) = player_finder.find_all() {
                let mut candidates_with_priority: Vec<_> = all_players
                    .into_iter()
                    .filter_map(|p| {
                        if p.bus_name() == "org.mpris.MediaPlayer2.playerctld" {
                            return None;
                        }

                        let identity = p.identity();
                        let bus_name = p.bus_name();
                        let allowlist_pos = settings.allowlist.iter()
                            .position(|pattern| {
                                matches_allowlist(identity, pattern) || matches_allowlist(bus_name, pattern)
                            })?;

                        let status_priority = get_playback_priority(&p);
                        let metadata_quality = p.get_metadata().ok()
                            .map(|meta| if has_valid_metadata(&meta) { 0 } else { 1 })
                            .unwrap_or(1);
                        // Use bus_name as final tiebreaker for deterministic selection
                        let bus_name_owned = bus_name.to_string();
                        Some((p, (status_priority, allowlist_pos, metadata_quality, bus_name_owned)))
                    })
                    .collect();

                if let Some(best_idx) = candidates_with_priority.iter()
                    .enumerate()
                    .min_by_key(|(_, (_, priority_tuple))| priority_tuple)
                    .map(|(idx, _)| idx)
                {
                    allowlist_finder = Ok(candidates_with_priority.swap_remove(best_idx).0);
                }
            }
            allowlist_finder
        } else {
            player_finder.find_active()
        };

        // Connect with player
        #[cfg(target_os = "linux")]
        let player = match selected_player {
            Ok(player) => {
                if player_notif != 1 {
                    println!("Found active player with MPRIS support.");
                    player_notif = 1;
                }
                player
            }
            Err(_) => {
                if player_notif != 2 {
                    if allowlist_enabled {
                        println!(
                            "Could not find any active player from your allowlist with MPRIS support. Waiting for any player from your allowlist..."
                        );
                    } else {
                        println!(
                            "Could not find any player with MPRIS support. Waiting for any player..."
                        );
                    }

                    player_notif = 2;
                    discord_notif = false;
                }

                is_interrupted = true;
                utils::clear_activity(&mut is_activity_set, &mut client);
                sleep(Duration::from_secs(interval));
                continue;
            }
        };

        // On macOS use media info fetching function to determine if anything is playing now
        #[cfg(target_os = "macos")]
        let player = match utils::get_currently_playing() {
            Ok(player) => {
                if allowlist_enabled {
                    let mut is_player_on_allowlist = false;
                    for allowlist_entry in &settings.allowlist {
                        if *allowlist_entry == player.player_id {
                            is_player_on_allowlist = true;
                            break;
                        }
                    }
                    if !is_player_on_allowlist {
                        if player_notif != 2 {
                            println!(
                            	"Could not find any active player from your allowlist. Waiting for any player from your allowlist..."
                            );
                            player_notif = 2;
                            discord_notif = false;
                        }

                        is_interrupted = true;
                        utils::clear_activity(&mut is_activity_set, &mut client);
                        sleep(Duration::from_secs(interval));
                        continue;
                    }
                }

                if player_notif != 1 {
                    println!("Found active player using media-control.");
                    player_notif = 1;
                }
                player
            }
            Err(e) => {
                if player_notif != 2 {
                    println!("{}", e);

                    player_notif = 2;
                    discord_notif = false;
                }

                is_interrupted = true;
                utils::clear_activity(&mut is_activity_set, &mut client);
                sleep(Duration::from_secs(interval));
                continue;
            }
        };

        #[cfg(target_os = "linux")]
        let mut player_name = player.identity().to_string();
        #[cfg(target_os = "linux")]
        let current_player_bus_name = player.bus_name().to_string();
        #[cfg(target_os = "macos")]
        let mut player_name = player.player_id.clone();

        // Use video presence if player is in video_players list
        let is_video_player = settings
            .video_players
            .iter()
            .any(|video_player_name| video_player_name == &player_name);
        if is_video_player {
            client = &mut client_video;
            debug_log!(settings.debug_log, "Using video player presence");
        } else {
            client = &mut client_audio;
            debug_log!(settings.debug_log, "Using audio player presence");
        }

        #[cfg(target_os = "macos")]
        {
            player_name = utils::app_name_from_bundle_id(player_name.as_str());
        }

        let mut player_id = utils::sanitize_name(&player_name);

        debug_log!(settings.debug_log, "player_name: {}", player_name);
        #[cfg(target_os = "linux")]
        debug_log!(settings.debug_log, "player_bus_name: {}", current_player_bus_name);
        debug_log!(settings.debug_log, "player_id: {}", player_id);
        debug_log!(
            settings.debug_log,
            "force_player_name: {}",
            force_player_name
        );
        debug_log!(settings.debug_log, "force_player_id: {}", force_player_id);

        // Display player ID and exit
        if settings.get_player_id {
            println!("\nplayer_id: {}", player_id);
            return Ok(());
        }

        #[cfg(target_os = "macos")]
        let last_player_id = player.player_id.clone();

        // Set different name and ID for RPC if enabled by argument
        if !force_player_name.is_empty() {
            player_name = force_player_name.to_string();
        }
        if !force_player_id.is_empty() {
            player_id = force_player_id.to_string();
        }

        // Connect with Discord
        if (is_first_time_audio && !is_video_player) || (is_first_time_video && is_video_player) {
            match client.connect() {
                Ok(_) => {
                    println!("Connected to Discord.");
                    discord_notif = false;
                }
                Err(_) => {
                    if !discord_notif {
                        println!("Could not connect to Discord. Waiting for discord to start...");
                        discord_notif = true;
                    }
                    sleep(Duration::from_secs(interval));
                    continue;
                }
            };
            if is_video_player {
                is_first_time_video = false;
            } else {
                is_first_time_audio = false;
            }
        } else {
            match client.reconnect() {
                Ok(_) => {
                    if discord_notif {
                        println!("Reconnected to Discord.");
                    }
                    is_interrupted = true;
                    discord_notif = false;
                }
                Err(_) => {
                    if !discord_notif {
                        println!("Could not reconnect to Discord. Waiting for discord to start...");
                        discord_notif = true;
                    }
                    sleep(Duration::from_secs(interval));
                    continue;
                }
            };
        }

        loop {
            debug_log!(
                settings.debug_log,
                "───────────────────────────────Loop─2───────────────────────────────────"
            );

            // Get metadata from player
            #[cfg(target_os = "linux")]
            let media_info = match utils::get_currently_playing(&player, settings.debug_log) {
                Ok(metadata) => metadata,
                Err(err) => {
                    println!("Could not get metadata from player: {}", err);
                    utils::clear_activity(&mut is_activity_set, &mut client);
                    break;
                }
            };
            #[cfg(target_os = "macos")]
            let media_info = match utils::get_currently_playing() {
                Ok(metadata) => metadata,
                Err(err) => {
                    println!("Could not get metadata from player: {}", err);
                    utils::clear_activity(&mut is_activity_set, &mut client);
                    break;
                }
            };
            debug_log!(settings.debug_log, "{:#?}", media_info);

            // Fix allowlist on macos, if player ID changes then break loop
            #[cfg(target_os = "macos")]
            if media_info.player_id != last_player_id {
                debug_log!(settings.debug_log, "Detected player change.");
                utils::clear_activity(&mut is_activity_set, client);
                break;
            }

            if !media_info.is_playing {
                is_interrupted = true;
                if settings.only_when_playing {
                    utils::clear_activity(&mut is_activity_set, client);
                    sleep(Duration::from_secs(interval));
                    continue;
                } else {
                    #[cfg(target_os = "linux")]
                    let should_reselect = {
                        if let Ok(all_players) = player_finder.find_all() {
                            all_players.into_iter().any(|p| {
                                if p.bus_name() == "org.mpris.MediaPlayer2.playerctld"
                                    || p.bus_name() == current_player_bus_name {
                                    return false;
                                }

                                // If allowlist is enabled, only consider players on the allowlist
                                if allowlist_enabled {
                                    if !is_player_allowlisted(&p, &settings.allowlist) {
                                        return false;
                                    }
                                }

                                if get_playback_priority(&p) != 0 {
                                    return false;
                                }

                                if let Ok(meta) = p.get_metadata() {
                                    has_valid_metadata(&meta)
                                } else {
                                    false
                                }
                            })
                        } else {
                            false
                        }
                    };
                    #[cfg(target_os = "macos")]
                    let should_reselect = false;

                    if should_reselect {
                        sleep(Duration::from_secs(interval));
                        break;
                    }
                }
            }

            let album_id = format!("{} - {}", media_info.album_artist, media_info.album);

            // If all metadata values are unknown then break
            if (media_info.artist.to_lowercase() == "unknown artist")
                && (media_info.album.to_lowercase() == "unknown album")
                && (media_info.title.to_lowercase() == "unknown title")
            {
                debug_log!(settings.debug_log, "Unknown metadata, skipping...");
                sleep(Duration::from_secs(interval));
                break;
            }

            // If artist or track is empty then break
            if (media_info.artist.len() == 0) | (media_info.title.len() == 0) {
                debug_log!(settings.debug_log, "Unknown metadata, skipping...");
                sleep(Duration::from_secs(interval));
                break;
            }

            let mut metadata_changed: bool = false;
            debug_log!(settings.debug_log, "Checking if metadata changed:");
            debug_log!(settings.debug_log, "{} - {last_title}", media_info.title);
            debug_log!(settings.debug_log, "{} - {last_album}", media_info.album);
            debug_log!(settings.debug_log, "{} - {last_artist}", media_info.artist);
            debug_log!(
                settings.debug_log,
                "{} - {last_album_artist}",
                media_info.album_artist
            );
            debug_log!(
                settings.debug_log,
                "is_playing: {} - {}",
                media_info.is_playing,
                last_is_playing
            );
            if (media_info.title != last_title)
                | (media_info.album != last_album)
                | (media_info.artist != last_artist)
                | (media_info.album_artist != last_album_artist)
                | (media_info.is_playing != last_is_playing)
            {
                metadata_changed = true;
            }

            debug_log!(
                settings.debug_log,
                "track_position: {} - {}",
                media_info.position,
                last_track_position
            );

            // Check if song repeated
            if (media_info.position < last_track_position) && !metadata_changed {
                debug_log!(settings.debug_log, "Detected a potential song seek/replay");
                metadata_changed = true;
            }
            last_track_position = media_info.position; // update it before loop continue
            debug_log!(settings.debug_log, "metadata_changed: {}", metadata_changed);

            if !metadata_changed && !is_interrupted {
                debug_log!(
                    settings.debug_log,
                    "The same metadata and status, skipping..."
                );

                sleep(Duration::from_secs(interval));
                continue;
            }

            // Get unix time of track start if supported, else return time now
            let time_start: u64 = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
                Ok(n) => n.as_secs().sub(media_info.position),
                Err(_) => 0,
            };

            // Fetch album cover
            if album_id != last_album_id {
                if lastfm_api_key.is_empty() {
                    _cover_url = "missing-cover".to_string()
                } else {
                    _cover_url = utils::get_cover_url(
                        &album_id,
                        media_info.album.as_str(),
                        _cover_url,
                        cache_enabled,
                        &mut album_cache,
                        media_info.album_artist.as_str(),
                        &lastfm_api_key,
                    );

                    // Fallback for Apple Music for album names with " - EP" and " - Single"
                    if _cover_url.is_empty() || _cover_url == "missing-cover" {
                        let album_name = media_info.album.trim();
                        let album_name_without_suffix = if album_name.ends_with(" - EP") {
                            &album_name[..album_name.len() - 5]
                        } else if album_name.ends_with(" - Single") {
                            &album_name[..album_name.len() - 9]
                        } else {
                            ""
                        };

                        if !album_name_without_suffix.is_empty() {
                            debug_log!(
                            settings.debug_log,
                            "Album cover not found, attempting to use album name without the 'EP' or 'Single' suffix (Apple Music)."
                            );
                            debug_log!(
                                settings.debug_log,
                                "{} => {}",
                                album_name,
                                album_name_without_suffix
                            );

                            _cover_url = utils::get_cover_url(
                                &album_id,
                                album_name_without_suffix,
                                _cover_url,
                                cache_enabled,
                                &mut album_cache,
                                media_info.album_artist.as_str(),
                                &lastfm_api_key,
                            );
                        }
                    }
                }

                // Use Musicbrainz cover if Last.fm fails
                if !settings.disable_musicbrainz_cover {
                    if _cover_url.is_empty() || _cover_url == "missing-cover" {
                        _cover_url = utils::get_cover_url_musicbrainz(
                            &album_id,
                            media_info.album.as_str(),
                            _cover_url,
                            cache_enabled,
                            &mut album_cache,
                            media_info.album_artist.as_str(),
                        );
                    }
                }
            }

            let image: String = if _cover_url.is_empty() || _cover_url == "missing-cover" {
                match media_info.art_url.is_empty() {
                    true => "missing-cover".to_string(),
                    false => {
                        if media_info.art_url.starts_with("http") && !settings.disable_mpris_art_url
                        {
                            media_info.art_url
                        } else {
                            "missing-cover".to_string()
                        }
                    }
                }
            } else {
                _cover_url.clone()
            };

            // Save last refresh info
            last_title = media_info.title.clone();
            last_album = media_info.album.clone();
            last_artist = media_info.artist.clone();
            last_album_artist = media_info.album_artist;
            last_album_id = album_id.to_string();
            last_is_playing = media_info.is_playing;

            // Set activity
            let song_name: String = format!("{} - {}", media_info.artist, media_info.title);
            let title = if media_info.title.len() > 1 {
                media_info.title
            } else {
                format!("{} ", media_info.title) // Discord activity min 2 char len bug fix
            };
            let artist = match rpc_name.as_str() {
                "artist" => {
                    if media_info.artist.len() > 1 {
                        media_info.artist
                    } else {
                        format!("{} ", media_info.artist) // Discord activity min 2 char len bug fix
                    }
                }
                _ => format!("by: {}", media_info.artist),
            };
            let album = format!("album: {}", media_info.album);
            let status_text: String = if media_info.is_playing {
                "playing".to_string()
            } else {
                "paused".to_string()
            };

            let mut assets = activity::Assets::new().large_image(&image);

            if !settings.hide_album_name {
                assets = assets.large_text(&album);
            }

            // Icon displayed next to the album cover
            match small_image.as_str() {
                "player" => {
                    if !settings.disable_mpris_art_url && image.contains("ytimg.com/") {
                        assets = assets.small_image("youtube").small_text("YouTube")
                    } else {
                        assets = assets.small_image(&player_id).small_text(&player_name)
                    }
                }
                "lastfmAvatar" => {
                    if !lastfm_avatar.is_empty() {
                        assets = assets
                            .small_image(&lastfm_avatar)
                            .small_text(&lastfm_icon_text);
                    }
                }
                "none" => {}
                _ => assets = assets.small_image(&status_text).small_text(&status_text),
            }

            // Display paused icon anyway if playpack is paused or stopped
            if status_text != "playing" {
                assets = assets.small_image(&status_text).small_text(&status_text)
            }

            let mut payload = activity::Activity::new()
                .details(&title)
                .assets(assets)
                .activity_type(if is_video_player {
                    activity::ActivityType::Watching
                } else {
                    activity::ActivityType::Listening
                });

            // "Listening to ..."
            match rpc_name.as_str() {
                "none" => payload = payload.status_display_type(StatusDisplayType::Name),
                "track" => payload = payload.status_display_type(StatusDisplayType::Details),
                "artist" | _ => payload = payload.status_display_type(StatusDisplayType::State),
            }

            // Don't display Unknown Artist for videos
            if !(is_video_player && (artist.to_lowercase() == "by: unknown artist")
                || artist.to_lowercase() == "unknown artist")
            {
                payload = payload.state(&artist);
            }

            payload = if media_info.is_track_position && (media_info.duration > 0) {
                let time_end = time_start + media_info.duration;
                if media_info.is_playing {
                    payload.timestamps(
                        activity::Timestamps::new()
                            .start(time_start.try_into().unwrap())
                            .end(time_end.try_into().unwrap()),
                    )
                } else {
                    payload.timestamps(
                        activity::Timestamps::new().start(time_start.try_into().unwrap()),
                    )
                }
            } else {
                payload.timestamps(activity::Timestamps::new().end(time_start.try_into().unwrap()))
            };

            // Create urls for activity links
            let yt_url: String = format!(
                "https://www.youtube.com/results?search_query={}",
                url_escape::encode_component(&song_name)
            );
            let lastfm_url: String = format!(
                "https://www.last.fm/user/{}",
                url_escape::encode_component(&lastfm_name)
            );
            let listenbrainz_url: String = format!(
                "https://listenbrainz.org/user/{}/",
                url_escape::encode_component(&listenbrainz_name)
            );

            // Add YouTube URL to song title
            payload = payload.details_url(&yt_url);

            // Add activity buttons
            let mut buttons = Vec::new();
            let mut first_button = "";
            for button in &settings.button {
                let initial_len = buttons.len();
                if initial_len == 2 {
                    break;
                }

                // Make sure buttons wont repeat
                if initial_len > 0 {
                    if first_button == button {
                        continue;
                    }
                }

                match button.as_str() {
                    "yt" => {
                        buttons.push(activity::Button::new(
                            "Search this song on YouTube",
                            &yt_url,
                        ));
                    }
                    "lastfm" => {
                        if lastfm_name.len() > 0 {
                            buttons.push(activity::Button::new("Last.fm profile", &lastfm_url));
                        }
                    }
                    "listenbrainz" => {
                        if listenbrainz_name.len() > 0 {
                            buttons.push(activity::Button::new(
                                "Listenbrainz profile",
                                &listenbrainz_url,
                            ));
                        }
                    }
                    "mprisUrl" => {
                        if media_info.url.is_empty() {
                            // if mpris url is empty or not set convert button to yt button
                            buttons.push(activity::Button::new(
                                "Search this song on YouTube",
                                &yt_url,
                            ));
                        } else {
                            if is_video_player {
                                buttons.push(activity::Button::new("Watch Now", &media_info.url));
                            } else {
                                buttons.push(activity::Button::new("Play Now", &media_info.url));
                            }
                        }
                    }
                    "shamelessAd" => {
                        buttons.push(activity::Button::new(
                            "Get This RPC",
                            "https://github.com/patryk-ku/music-discord-rpc",
                        ));
                    }
                    _ => continue,
                }

                // Make sure buttons wont repeat
                if initial_len < buttons.len() {
                    first_button = button;
                }
            }

            payload = match buttons.is_empty() {
                false => payload.buttons(buttons),
                true => payload,
            };

            match client.set_activity(payload) {
                Ok(_) => {
                    is_interrupted = false;
                    is_activity_set = true;
                    println!("=> Set activity [{status_text}]: {song_name}");
                }
                Err(_) => {
                    println!("Could not set activity.");
                    is_interrupted = true;
                    is_activity_set = false;
                    client.close()?;
                    break;
                }
            };

            sleep(Duration::from_secs(interval));
        }

        sleep(Duration::from_secs(interval));
    }
}
