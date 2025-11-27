use clap_serde_derive::{
    clap::{self, Parser, Subcommand},
    serde::Serialize,
    ClapSerde,
};
use std::fs;
use std::path::PathBuf;
use std::process;

use crate::debug_log;
use crate::utils::get_config_path;

#[derive(Parser, ClapSerde, Serialize, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Activity refresh rate (min: 5, default: 10)
    #[arg(short, long, value_name = "seconds", value_parser = clap::value_parser!(u64).range(5..))]
    pub interval: Option<u64>,

    /// Select visible buttons
    #[arg(short, long, value_name = "name", value_parser = ["yt", "lastfm", "listenbrainz", "mprisUrl", "shamelessAd"])]
    pub button: Vec<String>,

    /// Your Last.fm nickname
    #[arg(long, value_name = "nickname", value_parser = clap::value_parser!(String))]
    pub lastfm_name: Option<String>,

    /// Your Listenbrainz nickname
    #[arg(long, value_name = "nickname", value_parser = clap::value_parser!(String))]
    pub listenbrainz_name: Option<String>,

    /// Select what will be displayed after "Listening to" (default: artist)
    #[arg(short, long, value_name = "value", value_parser = ["artist", "track", "none"])]
    pub rpc_name: Option<String>,

    /// Select the icon displayed next to the album cover (default: playPause)
    #[arg(short, long, value_name = "name", value_parser = ["playPause", "player", "lastfmAvatar", "none"])]
    pub small_image: Option<String>,

    /// Force a different player id to be displayed than the one actually used
    #[arg(long, value_name = "player_id", value_parser = clap::value_parser!(String))]
    pub force_player_id: Option<String>,

    /// Force a different player name to be displayed than the one actually used
    #[arg(long, value_name = "player name", value_parser = clap::value_parser!(String))]
    pub force_player_name: Option<String>,

    /// Prevent MPRIS artUrl to be used as album cover if cover is not available on Last.fm
    #[arg(long)]
    pub disable_mpris_art_url: bool,

    /// Displays all available music player names and exits. Use to get your player name for -a argument
    #[arg(short, long)]
    #[serde(skip_deserializing)]
    pub list_players: bool,

    /// Show ID of currently detected player. Use when requesting missing icon.
    #[arg(long)]
    #[serde(skip_deserializing)]
    pub get_player_id: bool,

    /// Get status only from given player. Use multiple times to add several players.
    #[arg(short = 'a', long = "allowlist-add", value_name = "Player Name", value_parser = clap::value_parser!(String))]
    pub allowlist: Vec<String>,

    /// Will use the "watching" activity. Use multiple times to add several players.
    #[arg(short = 'w', long = "video-players", value_name = "Player Name", value_parser = clap::value_parser!(String))]
    pub video_players: Vec<String>,

    /// Hide album name
    #[arg(long)]
    pub hide_album_name: bool,

    /// Only send activity when media is playing
    #[arg(long)]
    pub only_when_playing: bool,

    /// Disable cache (not recommended)
    #[arg(short, long)]
    pub disable_cache: bool,

    /// Your Last.fm API key
    #[arg(long, value_name = "api_key", value_parser = clap::value_parser!(String))]
    pub lastfm_api_key: Option<String>,

    /// Do not use MusicBrainz as a fallback source of album covers
    #[arg(long)]
    pub disable_musicbrainz_cover: bool,

    /// Show debug log
    #[arg(long)]
    #[serde(skip_deserializing)]
    pub debug_log: bool,

    /// Reset config file (overwrites the old file if exists)
    #[arg(long)]
    #[serde(skip_deserializing)]
    pub reset_config: bool,

    /// Recursive fields
    #[serde(skip_deserializing)]
    #[command(flatten)]
    pub suboptions: SubConfig,
}

#[derive(Debug, Parser, Default, Serialize)]
pub struct SubConfig {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, Serialize)]
pub enum Commands {
    /// Start RPC in the background and enable autostart
    Enable {
        /// Use XDG Autostart instead of systemd
        #[arg(long)]
        #[serde(skip_deserializing)]
        xdg: bool,
    },
    /// Stop RPC and disable autostart
    Disable {
        /// Use XDG Autostart instead of systemd
        #[arg(long)]
        #[serde(skip_deserializing)]
        xdg: bool,
    },
    /// Use to restart the service and reload the changed configuration file.
    Restart {},
}

// Use to get config path, create new config or reset existing
fn create_config_file(force: bool) -> (bool, PathBuf) {
    let mut config_file = match get_config_path() {
        Some(path) => path,
        None => {
            println!("\x1b[31mWARNING: Failed to determine user config directory.\x1b[0m");
            return (false, PathBuf::new());
        }
    };
    config_file.push("music-discord-rpc");

    let config_dir = config_file.clone();
    config_file.push("config.yaml");

    if config_file.exists() && !force {
        return (true, config_file);
    }

    let config_text = r#"# music-discord-rpc configuration file

# You can reset this file using the command:
# music-discord-rpc --reset-config
# Or you can manually copy the example config from repo:
# https://github.com/patryk-ku/music-discord-rpc/blob/main/config.yaml

# If you compiled binary by yourself, you may need to provide your Last.fm API key here.
# Or if you use precompiled binary, you can override the default Last.fm API key.
# You can easily get it from: https://www.last.fm/pl/api
# lastfm_api_key: key_here

# You can also disable Last.fm as a cover source by providing an empty string as the key.
# lastfm_api_key: ""

# Activity refresh rate in seconds (min 5)
interval: 10

# Select visible activity buttons (max 2) [possible values: yt, lastfm, listenbrainz, mprisUrl, shamelessAd]
# button:
#   - yt
#   - lastfm

# Uncomment and enter your nicknames for activity buttons
# lastfm_name: "nickname"
# listenbrainz_name: "nickname"

# Select what will be displayed after "Listening to" (default: artist) [possible values: artist, track, none]
# rpc_name: artist

# Select the icon displayed next to the album cover (default playPause) [possible values: playPause, player, lastfmAvatar, none]
small_image: playPause

# Force a different player id and name to be displayed than the one actually used. "force_player_id" changes icon and "force_player_name" changes displayed text while hovering over the icon.
# List of available icons: https://github.com/patryk-ku/music-discord-rpc?tab=readme-ov-file#the-icon-next-to-the-album-cover
# force_player_id: "custom_player_id"
# force_player_name: "Custom Player Name"

# Prevent MPRIS artUrl to be used as album cover if cover is not available on Last.fm. Mainly for working with thumbnails from YouTube and other video sites.
# Additionally, it also disables icon and player name replacement on YouTube if it detects a YouTube thumbnail link.
disable_mpris_art_url: false

# Only use the status from the following music players
# Use -l, --list-players to get player exact name to use with this option
# The order matters and the first is the most important.
# You can use wildcard (*) at the end of a pattern to match any player name starting with that prefix.
# allowlist:
#   - "VLC Media Player"
#   - "Chrome"
#   - "org.mpris.MediaPlayer2.kdeconnect.mpris_*"

# Will use the "watching" activity
# Use -l, --list-players to get player exact name to use with this option
# video_players:
#   - "VLC Media Player"
#   - "Chrome"

# Hide the album name to decrease activity height
hide_album_name: false

# Only send activity when media is playing
only_when_playing: false

# Prevent MusicBrainz to be used as source of album cover if cover is not available on Last.fm
disable_musicbrainz_cover: false

# Disable cache (not recommended)
disable_cache: false
"#;

    match fs::create_dir_all(&config_dir) {
        Err(_) => {
            println!("[config] Failed to create config directory.");
            return (false, config_file);
        }
        Ok(_) => match fs::write(&config_file, config_text) {
            Ok(_) => println!(
                "[config] Created new config file: {}",
                config_file.display()
            ),
            Err(_) => {
                println!("[config] Error: Failed to create config file.");
                return (false, config_file);
            }
        },
    }

    return (true, config_file);
}

// Used to get settings merged from args and config file
pub fn load_settings() -> Cli {
    let args = Cli::parse();
    debug_log!(args.debug_log, "Debug logs: enabled.");
    debug_log!(args.debug_log, "args: {:#?}", args);

    // Reset config file is user used --reset-config and exit
    if args.reset_config {
        create_config_file(true);
        process::exit(0);
    }

    let (mut config_exists, config_file) = create_config_file(false);
    if !config_exists {
        return args;
    }

    // Read user config file
    let mut config = match fs::read_to_string(&config_file) {
        Ok(yaml_str) => match serde_yaml::from_str::<<Cli as ClapSerde>::Opt>(&yaml_str) {
            Ok(yaml_args) => Cli::from(yaml_args),
            Err(error) => {
                println!("Failed to parse config file: {}", error);
                config_exists = false;
                Cli::from_clap()
            }
        },
        Err(_) => {
            println!("Failed to read config file.");
            config_exists = false;
            Cli::from_clap()
        }
    };

    if !config_exists {
        return args;
    }
    println!("Configuration loaded from file: {}", config_file.display());
    debug_log!(args.debug_log, "config: {:#?}", config);

    // Logic of merging config with args
    if args.interval != config.interval && args.interval.is_some() {
        config.interval = args.interval;
    }

    if args.button != config.button && args.button.len() > 0 {
        config.button = args.button;
    }

    if args.lastfm_name != config.lastfm_name && args.lastfm_name.is_some() {
        config.lastfm_name = args.lastfm_name;
    }

    if args.listenbrainz_name != config.listenbrainz_name && args.listenbrainz_name.is_some() {
        config.listenbrainz_name = args.listenbrainz_name;
    }

    if args.rpc_name != config.rpc_name && args.rpc_name.is_some() {
        config.rpc_name = args.rpc_name;
    }

    if args.small_image != config.small_image && args.small_image.is_some() {
        config.small_image = args.small_image;
    }

    if args.force_player_id != config.force_player_id && args.force_player_id.is_some() {
        config.force_player_id = args.force_player_id;
    }

    if args.force_player_name != config.force_player_name && args.force_player_name.is_some() {
        config.force_player_name = args.force_player_name;
    }

    if args.disable_musicbrainz_cover {
        config.disable_musicbrainz_cover = args.disable_musicbrainz_cover;
    }

    if args.hide_album_name {
        config.hide_album_name = args.hide_album_name;
    }

    if args.only_when_playing {
        config.only_when_playing = args.only_when_playing;
    }

    if args.disable_cache {
        config.disable_cache = args.disable_cache;
    }

    if args.list_players {
        config.list_players = args.list_players;
    }

    if args.get_player_id {
        config.get_player_id = args.get_player_id;
    }

    if args.allowlist != config.allowlist && args.allowlist.len() > 0 {
        config.allowlist = args.allowlist;
    }

    if args.video_players != config.video_players && args.video_players.len() > 0 {
        config.video_players = args.video_players;
    }

    if args.lastfm_api_key != config.lastfm_api_key && args.lastfm_api_key.is_some() {
        config.lastfm_api_key = args.lastfm_api_key;
    }

    if args.disable_mpris_art_url {
        config.disable_mpris_art_url = args.disable_mpris_art_url;
    }

    if args.debug_log {
        config.debug_log = args.debug_log;
    }

    if args.reset_config {
        config.reset_config = args.reset_config;
    }

    config.suboptions = args.suboptions;

    return config;
}
