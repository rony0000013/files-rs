//! Play All Service
//! Provides functionality to queue and play all media files in a selection
//! Uses Windows API: ShellExecuteW for launching media player with playlist

use crate::core::path_util::FilesPath;
use anyhow::{Context, Result};
use std::path::Path;

/// Media playback mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackMode {
    /// Play all files sequentially in default player
    Sequential,
    /// Shuffle and play all files
    Shuffle,
    /// Loop through all files repeatedly
    Loop,
}

impl Default for PlaybackMode {
    fn default() -> Self {
        Self::Sequential
    }
}

/// Result of a play all operation
#[derive(Debug)]
pub struct PlayAllResult {
    pub success: bool,
    pub files_queued: u32,
    pub player_launched: bool,
    pub error_message: Option<String>,
}

/// Service for playing all media files in a selection
pub struct PlayAllService;

impl PlayAllService {
    /// Creates a new PlayAllService instance
    pub fn new() -> Self {
        Self
    }

    /// Supported media file extensions
    fn is_media_file(path: &Path) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        matches!(
            ext.as_str(),
            "mp3"
                | "wav"
                | "flac"
                | "aac"
                | "m4a"
                | "ogg"
                | "wma"
                | "ape"
                | "opus"
                | "mp4"
                | "avi"
                | "mkv"
                | "mov"
                | "wmv"
                | "flv"
                | "webm"
                | "m4v"
                | "mpg"
                | "mpeg"
                | "3gp"
        )
    }

    /// Filters a list of paths to only include media files
    pub fn filter_media_files(paths: &[FilesPath]) -> Vec<FilesPath> {
        paths
            .iter()
            .filter(|p| Self::is_media_file(p.as_path()))
            .cloned()
            .collect()
    }

    /// Plays all media files in the given selection using the default player
    #[cfg(windows)]
    pub async fn play_all(&self, paths: &[FilesPath], mode: PlaybackMode) -> Result<PlayAllResult> {
        use std::process::Command;

        // Filter to only media files
        let media_files = Self::filter_media_files(paths);

        if media_files.is_empty() {
            return Ok(PlayAllResult {
                success: false,
                files_queued: 0,
                player_launched: false,
                error_message: Some("No media files found in selection".to_string()),
            });
        }

        let file_count = media_files.len() as u32;
        tracing::info!("Playing {} media files in {:?} mode", file_count, mode);

        // On Windows, use PowerShell to play with Windows Media Player or VLC if installed
        // For a playlist, we can use mplayer's playlist feature or create a temporary m3u file

        let temp_dir = std::env::temp_dir();
        let playlist_path = temp_dir.join("files_rs_playlist.m3u");

        // Write playlist file
        let playlist_content: String = media_files
            .iter()
            .map(|p| p.as_path().to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("\n");

        std::fs::write(&playlist_path, playlist_content)
            .context("Failed to create playlist file")?;

        // Try to launch with VLC if available, otherwise use explorer
        let vlc_paths = [
            "C:\\Program Files\\VideoLAN\\VLC\\vlc.exe",
            "C:\\Program Files (x86)\\VideoLAN\\VLC\\vlc.exe",
        ];

        let mut player_launched = false;

        for vlc_path in &vlc_paths {
            let vlc = Path::new(vlc_path);
            if vlc.exists() {
                let mut cmd = Command::new(vlc);
                match mode {
                    PlaybackMode::Sequential => {
                        cmd.arg("--playlist-start=0");
                    }
                    PlaybackMode::Shuffle => {
                        cmd.arg("--random");
                    }
                    PlaybackMode::Loop => {
                        cmd.arg("--loop");
                    }
                }
                cmd.arg("--playlist-enqueue");
                cmd.arg(playlist_path.as_path());
                cmd.stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());

                match cmd.spawn() {
                    Ok(_) => {
                        player_launched = true;
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to launch VLC: {}", e);
                    }
                }
            }
        }

        // If VLC not available, use explorer's media player via ShellExecuteW
        if !player_launched {
            use std::ffi::OsStr;
            use std::os::windows::ffi::OsStrExt;

            let path_wide: Vec<u16> = OsStr::new(playlist_path.to_str().unwrap_or(""))
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            unsafe {
                use windows::Win32::UI::Shell::ShellExecuteW;
                use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

                let result = ShellExecuteW(
                    None,
                    windows::core::PCWSTR::null(),
                    windows::core::PCWSTR(path_wide.as_ptr()),
                    None,
                    None,
                    SW_SHOW,
                );

                player_launched = result.0 as usize > 32;
            }
        }

        // Clean up temp playlist
        let _ = std::fs::remove_file(&playlist_path);

        Ok(PlayAllResult {
            success: player_launched,
            files_queued: file_count,
            player_launched,
            error_message: if player_launched {
                None
            } else {
                Some("Failed to launch media player".to_string())
            },
        })
    }

    /// Plays all media files (non-Windows stub)
    #[cfg(not(windows))]
    pub async fn play_all(&self, paths: &[FilesPath], mode: PlaybackMode) -> Result<PlayAllResult> {
        let _ = paths;
        let _ = mode;

        Ok(PlayAllResult {
            success: false,
            files_queued: 0,
            player_launched: false,
            error_message: Some("PlayAll is only available on Windows".to_string()),
        })
    }

    /// Gets the count of media files in a selection without playing them
    pub fn get_media_file_count(paths: &[FilesPath]) -> u32 {
        Self::filter_media_files(paths).len() as u32
    }
}

impl Default for PlayAllService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_is_media_file_audio() {
        let audio_extensions = ["mp3", "wav", "flac", "aac", "m4a", "ogg"];
        for ext in audio_extensions {
            let temp_dir = tempfile::tempdir().unwrap();
            let path = temp_dir.path().join(format!("song.{}", ext));
            assert!(
                PlayAllService::is_media_file(&path),
                "Expected {} to be recognized as audio",
                ext
            );
        }
    }

    #[test]
    fn test_is_media_file_video() {
        let video_extensions = ["mp4", "avi", "mkv", "mov", "wmv", "flv", "webm"];
        for ext in video_extensions {
            let temp_dir = tempfile::tempdir().unwrap();
            let path = temp_dir.path().join(format!("video.{}", ext));
            assert!(
                PlayAllService::is_media_file(&path),
                "Expected {} to be recognized as video",
                ext
            );
        }
    }

    #[test]
    fn test_is_not_media_file() {
        let non_media = ["txt", "pdf", "doc", "jpg", "png", "exe", "rs"];
        for ext in non_media {
            let temp_dir = tempfile::tempdir().unwrap();
            let path = temp_dir.path().join(format!("file.{}", ext));
            assert!(
                !PlayAllService::is_media_file(&path),
                "Expected {} to NOT be recognized as media",
                ext
            );
        }
    }

    #[test]
    fn test_is_media_file_case_insensitive() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("song.MP3");
        assert!(PlayAllService::is_media_file(&path));
    }

    #[test]
    fn test_filter_media_files() {
        let temp_dir = tempdir().unwrap();
        let audio_file = temp_dir.path().join("song.mp3");
        let video_file = temp_dir.path().join("video.mp4");
        let text_file = temp_dir.path().join("readme.txt");

        std::fs::write(&audio_file, "audio").unwrap();
        std::fs::write(&video_file, "video").unwrap();
        std::fs::write(&text_file, "text").unwrap();

        let paths = vec![
            FilesPath::new(&audio_file),
            FilesPath::new(&video_file),
            FilesPath::new(&text_file),
        ];

        let media_files = PlayAllService::filter_media_files(&paths);
        assert_eq!(media_files.len(), 2);
    }

    #[test]
    fn test_filter_media_files_none() {
        let temp_dir = tempdir().unwrap();
        let text_file = temp_dir.path().join("readme.txt");
        std::fs::write(&text_file, "text").unwrap();

        let paths = vec![FilesPath::new(&text_file)];
        let media_files = PlayAllService::filter_media_files(&paths);
        assert_eq!(media_files.len(), 0);
    }

    #[test]
    fn test_get_media_file_count() {
        let temp_dir = tempdir().unwrap();
        let mp3_file = temp_dir.path().join("song.mp3");
        let wav_file = temp_dir.path().join("audio.wav");
        let txt_file = temp_dir.path().join("readme.txt");

        std::fs::write(&mp3_file, "mp3").unwrap();
        std::fs::write(&wav_file, "wav").unwrap();
        std::fs::write(&txt_file, "txt").unwrap();

        let paths = vec![
            FilesPath::new(&mp3_file),
            FilesPath::new(&wav_file),
            FilesPath::new(&txt_file),
        ];

        assert_eq!(PlayAllService::get_media_file_count(&paths), 2);
    }

    #[test]
    fn test_get_media_file_count_empty() {
        let paths: Vec<FilesPath> = vec![];
        assert_eq!(PlayAllService::get_media_file_count(&paths), 0);
    }

    #[test]
    fn test_playback_mode_default() {
        assert_eq!(PlaybackMode::default(), PlaybackMode::Sequential);
    }

    #[test]
    fn test_playback_mode_variants() {
        let modes = [
            PlaybackMode::Sequential,
            PlaybackMode::Shuffle,
            PlaybackMode::Loop,
        ];
        for mode in modes {
            let debug_str = format!("{:?}", mode);
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn test_play_all_result_debug() {
        let result = PlayAllResult {
            success: true,
            files_queued: 10,
            player_launched: true,
            error_message: None,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("10"));
        assert!(debug_str.contains("true"));
    }

    #[test]
    fn test_service_creation() {
        let service = PlayAllService::new();
        let default_service = PlayAllService::default();
        assert!(true); // Both created successfully
        let _ = default_service;
        let _ = service;
    }
}
