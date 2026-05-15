use inquire::{list_option::ListOption, validator::Validation, Confirm, MultiSelect, Select, Text};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, process};

use crate::settings::create_config_file;
use crate::utils;

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    interval: u64,
    button: Vec<String>,
    lastfm_name: String,
    listenbrainz_name: String,
    rpc_name: String,
    small_image: String,
    disable_mpris_art_url: bool,
    allowlist: Vec<String>,
    video_players: Vec<String>,
    hide_album_name: bool,
    only_when_playing: bool,
    disable_musicbrainz_cover: bool,
}

pub fn setup() {
    let (config_exists, config_file) = create_config_file(false);
    if !config_exists {
        process::exit(1);
    }

    // Load existing config or set to default values
    let mut config = load_config(&config_file).unwrap_or(Config {
        interval: 10,
        button: vec![],
        lastfm_name: String::new(),
        listenbrainz_name: String::new(),
        rpc_name: "artist".into(),
        small_image: "player".into(),
        disable_mpris_art_url: false,
        allowlist: vec![],
        video_players: vec![],
        hide_album_name: true,
        only_when_playing: true,
        disable_musicbrainz_cover: true,
    });

    println!("\nmusic-discord-rpc config editor");
    println!("───────────────────────────────");
    println!("Here you can quickly update your settings.");
    println!("Nothing is saved until you confirm changes.");
    println!("Before editing, consider making a backup of your current config.");
    println!("Use arrows to navigate, Ctrl+C to exit.");

    if let Err(err) = config_form(&mut config) {
        eprintln!("Error: {}", err);
        process::exit(1);
    }

    // Save config
    let yaml = serde_yaml::to_string(&config).unwrap_or_else(|err| {
        eprintln!("Error serializing config: {}", err);
        process::exit(1);
    });

    // println!("\n──────── config preview ────────\n");
    // println!("{}", yaml);
    // println!("────────────────────────────────\n");

    println!("\nWarning: If exists your previous config will be overwritten.");
    let save = Confirm::new("Save this configuration?")
        .with_default(true)
        .prompt();

    match save {
        Ok(true) => {
            if let Err(err) = fs::write(config_file, yaml) {
                eprintln!("Error writing config file: {}", err);
                process::exit(1);
            }
            println!("Config saved.");
        }
        _ => {
            println!("Discarded.");
            process::exit(0);
        }
    }

    println!(
        "Tip: To restore default config with additional options and comments use --reset-config."
    );

    #[cfg(target_os = "linux")]
    {
        println!("\nNow attempting to restart and enable the systemd service.");
        println!(
            "If no error message appears, you are all set up and no further action is required."
        );

        // Reload and start
        match process::Command::new("systemctl")
            .arg("--user")
            .arg("restart")
            .arg("music-discord-rpc.service")
            .status()
        {
            Ok(_) => {
                println!("Restarted user systemd service.");
                utils::enable_service();
            }
            Err(_) => {
                println!("Failed to restart user systemd service.");
                process::exit(1);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        println!("Now run this command to reload service and apply the changes:");
        println!("brew services restart music-discord-rpc");
    }

    process::exit(0);
}

fn load_config(config_path: &PathBuf) -> Option<Config> {
    let content = fs::read_to_string(config_path).ok()?;
    serde_yaml::from_str(&content).ok()
}

fn config_form(config: &mut Config) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n────── main settings ──────");
    // Interval
    config.interval = Text::new("Activity refresh rate:")
        .with_help_message("In seconds (min: 5)")
        .with_initial_value(&config.interval.to_string())
        .with_validator(|value: &str| match value.parse::<u64>() {
            Ok(v) if v >= 5 => Ok(Validation::Valid),
            Ok(_) => Ok(Validation::Invalid("Value must be at least 5".into())),
            Err(_) => Ok(Validation::Invalid("Please enter a valid number".into())),
        })
        .prompt()?
        .parse()?;

    // Buttons
    let options = vec![
        "yt".to_string(),
        "lastfm".to_string(),
        "listenbrainz".to_string(),
        "mprisUrl".to_string(),
        "shamelessAd".to_string(),
    ];

    config.button = MultiSelect::new("Activity buttons (max 2):", options)
        .with_validator(|choices: &[ListOption<&String>]| {
            Ok(if choices.len() <= 2 {
                Validation::Valid
            } else {
                Validation::Invalid("Max 2 options".into())
            })
        })
        .prompt()?;

    if config.button.iter().any(|v| v == "lastfm") {
        config.lastfm_name = Text::new("Last.fm username:")
            .with_initial_value(&config.lastfm_name)
            .prompt()?;
    }

    if config.button.iter().any(|v| v == "listenbrainz") {
        config.listenbrainz_name = Text::new("ListenBrainz username:")
            .with_initial_value(&config.listenbrainz_name)
            .prompt()?;
    }

    // RPC name
    let rpc_names = vec!["artist", "track", "none"];

    config.rpc_name = Select::new("RPC name:", rpc_names)
        .with_help_message("Select what will be displayed after \"Listening to\"")
        .prompt()?
        .to_string();

    // Small icon
    let icons = vec!["player", "playPause", "lastfmAvatar", "none"];

    config.small_image = Select::new("Small icon:", icons)
        .with_help_message("Select the icon displayed next to the album cover")
        .prompt()?
        .to_string();

    if config.small_image == "lastfmAvatar" && config.lastfm_name.is_empty() {
        config.lastfm_name = Text::new("Last.fm username:")
            .with_initial_value(&config.lastfm_name)
            .prompt()?;
    }

    // MPRIS art url
    // config.disable_mpris_art_url = Confirm::new("Disable MPRIS art url?")
    // .with_help_message("Prevent MPRIS artUrl to be used as album cover if cover is not available on Last.fm. Mainly for working with thumbnails from YouTube and other video sites.")
    //     .with_default(config.disable_mpris_art_url)
    //     .prompt()?;

    // Album name
    config.hide_album_name = Confirm::new("Hide the album name?")
        .with_help_message("Hide the album name to decrease activity height")
        .with_default(config.hide_album_name)
        .prompt()?;

    // Only when playing
    config.only_when_playing = Confirm::new("Send activity only when media is playing?")
        .with_default(config.only_when_playing)
        .prompt()?;

    // ListenBrainz cover
    config.disable_musicbrainz_cover = Confirm::new("Disable ListenBrainz album covers?")
        .with_help_message("Prevent MusicBrainz to be used as source of album cover if cover is not available on Last.fm")
        .with_default(config.disable_musicbrainz_cover)
        .prompt()?;

    // Allowlist
    println!("\n──────── allowlist ────────");
    println!("Only use the status from the following music players.");
    println!(
        "Open new terminal and use -l or --list-players to get player exact name to use with this option."
    );
    println!("The order matters and the first is the most important.");
    config.allowlist = prompt_strings(&config.allowlist)?;

    // Watching Activity
    println!("\n────── video players ──────");
    println!("Selected players will use the \"watching\" activity instead of \"listening\".");
    println!(
        "Open new terminal and use -l or --list-players to get player exact name to use with this option."
    );
    config.video_players = prompt_strings(&config.video_players)?;
    Ok(())
}

fn prompt_strings(previous: &Vec<String>) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    if previous.len() > 0 {
        println!("\nPreviously selected options:");

        for (_i, item) in previous.iter().enumerate() {
            println!(" - {}", item);
        }
    }

    let mut items = Vec::new();

    println!("\n[Leave empty to stop]");
    loop {
        let value = Text::new("Enter value:").prompt()?;

        if value.trim().is_empty() {
            break;
        }

        items.push(value);
    }

    Ok(items)
}
