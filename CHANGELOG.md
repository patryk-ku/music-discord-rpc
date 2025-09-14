## v0.6.2

- Fix: Prevent playerctld from breaking allow list (#41).

## v0.6.1

- Fix the allowlist on macOS (#38).
- Add fallback cover search for albums with ` - EP` and ` - Single`  suffixes added by Apple Music (#39).
- Minor changes to how the player ID and name are determined (mainly on macOS).

## v0.6.0

- Ported to MacOS.
- Renamed project from **mpris-discord-rpc** to **music-discord-rpc**.
- Added `--get-player-id` flag for easier retrieval of Player ID when requesting missing icons.

## v0.5.1

- Added `--only-when-playing` argument and `only_when_playing` option in the config file. When enabled, RPC activity is only sent to Discord when media is playing, similar to Spotify. Thanks to @aritsune for the contribution.

## v0.5.0

- Added option to customize "Listening to" text on the Discord user list. It now looks the same as the Spotify RPC, but you can also choose to show either the artist name or the song title.
- The song title is now a link that searches for the song on YouTube.
- Minor bug fixes.
- Updated dependencies to latest versions.

## v0.4.0

- Added the ability to provide your own Last.fm API key via arguments and config file.
- Providing a Last.fm API key during compilation is now optional.
- Added MusicBrainz as a fallback source for album art when Last.fm doesn't provide one or API key is not set. This can be disabled with an argument or in the config.
- Added `--xdg` flag to `enable` and `disable` subcommands that creates or removes a .desktop file for XDG Autostart as an alternative to systemd for distributions without it.
- Now respects `$XDG_CONFIG_HOME` when choosing where to store and read the config file.
- Also available as an `.AppImage` format.

## v0.3.0

- If no album art is found on Last.fm, use the `artUrl` provided by MPRIS if it exists. This is especially useful for movies played in a browser, e.g., YouTube. If a YouTube thumbnail URL is detected, replace the player icon with the YouTube icon. This can be disabled with an argument or in the config.
- Added the option to mark players as video players, which will display the status "Watching Video" and make the RPC more appropriate for movies. A video thumbnail will be displayed if available as `artUrl` in MPRIS.
- Added a new `mprisUrl` button that can link to the currently playing content if MPRIS provides such information.
- The systemd unit file is now installed by the package manager instead of manually by the program.
- Added the ability to force a different player icon and name than is actually used.

## v0.2.2

- Fixed: Incorrect album art display when the album artist differed from the track artist.
- Updated dependencies to latest versions.

## v0.2.1

- Added option to hide album title in activity.
- Added the ability to customize displayed icon adjacent to album artwork. Available options: none, play/pause icons, music player icon, Last.fm avatar.

## v0.2.0

- (⚠️ **Breaking**) The `-n`, `--player-name` argument has been removed. Use `-a`, `--allowlist-add` instead.
- (⚠️ **Breaking**) The previous button arguments (`-p`, `--profile-button`, `-y`, `--yt-button`) have been consolidated into a single new argument: `-b, --button` with options: `yt`, `lastfm`, `listenbrainz` and separate arguments for setting service usernames. More additional buttons coming in the future.
- Added support for configuring the program via a configuration file.
- Added commands for easy setup of autostart using systemd.
- Now available as `.deb` and `.rpm` packages, and in the AUR.
- Print debug logs with `--debug-log`.

## v0.1.5

- Set Discord RPC activity type to "Listening".
- Listening progress bar similar to Spofity.
- Album name is now displayed.
- Fixed detection of track duration, current position and start/end time calculation.
- From now on, the program checks if the directory to which it tries to save the cache file exists. The cache should now work properly.

## v0.1.4

- Allowlist of music players (`-a` or `--allowlist-add`).

## v0.1.3

- Dependencies update.

## v0.1.2

- Dependencies update.

## v0.1.1

- List active MPRIS2 players with `-l` or `--list-players` arguments.
- Select only one specific player for music status with `-n` or `--player-name` arguments.
- Fix: skip setting status when unknown metadata.
- Better log messages.
- Switched to different cache library.

## v0.1.0

- Initial release.
