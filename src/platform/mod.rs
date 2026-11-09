// A common struct to hold song information, ensuring a consistent
// return type regardless of the platform.
#[derive(Debug, Clone)]
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
pub type NowPlayingResult = Result<MediaInfo, Box<dyn std::error::Error>>;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;
