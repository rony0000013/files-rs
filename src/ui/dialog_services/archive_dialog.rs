//! Archive Dialog Service
//! State machine for CreateArchiveDialog and DecompressArchiveDialog.
//! Handles archive format selection, compression level, password, and LZMA2 tuning.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Archive format types supported
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchiveFormat {
    Zip,
    SevenZ,
    TarGz,
}

impl ArchiveFormat {
    /// Returns the file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            ArchiveFormat::Zip => ".zip",
            ArchiveFormat::SevenZ => ".7z",
            ArchiveFormat::TarGz => ".tar.gz",
        }
    }

    /// Returns all available formats
    pub fn all() -> &'static [ArchiveFormat] {
        &[
            ArchiveFormat::Zip,
            ArchiveFormat::SevenZ,
            ArchiveFormat::TarGz,
        ]
    }
}

/// Compression level (0-9 for zip, 0-9 for 7z LZMA2)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompressionLevel(pub u32);

impl CompressionLevel {
    pub const MIN: u32 = 0;
    pub const MAX: u32 = 9;
    pub const STORE: u32 = 0;
    pub const FAST: u32 = 1;
    pub const DEFAULT: u32 = 5;
    pub const MAXIMUM: u32 = 9;

    pub fn new(level: u32) -> Self {
        Self(level.clamp(Self::MIN, Self::MAX))
    }

    pub fn description(&self) -> &'static str {
        match self.0 {
            0 => "Store (no compression)",
            1 => "Fastest",
            2 => "Fast",
            3 => "Normal",
            4 => "Better",
            5 => "Default",
            6 => "Better",
            7 => "Strong",
            8 => "Stronger",
            9 => "Maximum",
            _ => "Default",
        }
    }
}

/// LZMA2 tuning parameters (for 7z format)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lzma2Settings {
    /// Dictionary size in bytes (multiples of 2)
    pub dictionary_size: u32,
    /// Word size for LZMA
    pub word_size: u32,
    /// Number of CPU threads to use (0 = auto)
    pub cpu_threads: u32,
}

impl Default for Lzma2Settings {
    fn default() -> Self {
        Self {
            dictionary_size: 16,  // 2^16 = 64KB default
            word_size: 32,
            cpu_threads: 0,        // Auto-detect
        }
    }
}

impl Lzma2Settings {
    /// Validate dictionary size (must be power of 2 >= 2^16 and <= 2^27)
    pub fn is_valid_dictionary(&self) -> bool {
        self.dictionary_size >= 16 && self.dictionary_size <= 27
    }

    /// Validate word size (must be between 5 and 273)
    pub fn is_valid_word_size(&self) -> bool {
        self.word_size >= 5 && self.word_size <= 273
    }

    /// Get actual dictionary size in bytes
    pub fn dictionary_bytes(&self) -> usize {
        2u32.pow(self.dictionary_size) as usize
    }
}

/// Volume splitting for large archives (CD=700MB, DVD=4.7GB, FAT32=4GB)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VolumeSize {
    None,
    Cdrom,      // 700 MB
    Dvd,        // 4.7 GB
    Fat32,      // 4 GB - actual limit ~4,194,304 KB
    Custom(u64), // bytes
}

impl VolumeSize {
    pub fn bytes(&self) -> Option<u64> {
        match self {
            VolumeSize::None => None,
            VolumeSize::Cdrom => Some(700 * 1024 * 1024),
            VolumeSize::Dvd => Some(4_700_000_000),
            VolumeSize::Fat32 => Some(4_194_304 * 1024), // 4GB in bytes
            VolumeSize::Custom(size) => Some(*size),
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            VolumeSize::None => "No splitting",
            VolumeSize::Cdrom => "CD (700 MB)",
            VolumeSize::Dvd => "DVD (4.7 GB)",
            VolumeSize::Fat32 => "FAT32 (4 GB)",
            VolumeSize::Custom(_) => "Custom size",
        }
    }
}

/// State for CreateArchiveDialog
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateArchiveState {
    /// Archive format
    pub format: ArchiveFormat,
    /// Source items to archive (paths)
    pub source_paths: Vec<PathBuf>,
    /// Destination path (without extension)
    pub destination_base: PathBuf,
    /// Compression level
    pub level: CompressionLevel,
    /// Optional password for encryption
    pub password: Option<String>,
    /// Enable encryption
    pub encrypt: bool,
    /// LZMA2 settings (only used for 7z)
    pub lzma2: Lzma2Settings,
    /// Volume splitting
    pub volume: VolumeSize,
    /// Estimated output size (calculated async)
    pub estimated_size: Option<u64>,
    /// Show LZMA2 advanced options
    pub show_advanced: bool,
    /// Error message if validation failed
    pub error: Option<String>,
}

impl Default for CreateArchiveState {
    fn default() -> Self {
        Self {
            format: ArchiveFormat::Zip,
            source_paths: Vec::new(),
            destination_base: PathBuf::new(),
            level: CompressionLevel(CompressionLevel::DEFAULT),
            password: None,
            encrypt: false,
            lzma2: Lzma2Settings::default(),
            volume: VolumeSize::None,
            estimated_size: None,
            show_advanced: false,
            error: None,
        }
    }
}

impl CreateArchiveState {
    /// Create a new archive state with given sources
    pub fn new(sources: Vec<PathBuf>, destination: PathBuf) -> Self {
        Self {
            source_paths: sources,
            destination_base: destination,
            ..Default::default()
        }
    }

    /// Get the full destination path with extension
    pub fn destination_path(&self) -> PathBuf {
        let mut path = self.destination_base.clone();
        // Replace extension with selected format's extension
        if let Some(ext) = path.extension() {
            let mut new_ext = ext.to_os_string();
            new_ext.push(self.format.extension());
            path.set_extension(new_ext);
        } else {
            path.set_extension(self.format.extension().trim_start_matches('.'));
        }
        path
    }

    /// Validate the archive settings
    pub fn validate(&self) -> Result<(), String> {
        // Validate encryption requirements first (user-facing)
        if self.encrypt {
            if self.password.is_none() || self.password.as_ref().map_or(true, |p| p.is_empty()) {
                return Err("Password is required when encryption is enabled".to_string());
            }
        }
        // Check if source paths are provided
        if self.source_paths.is_empty() {
            return Err("No files selected for archiving".to_string());
        }
        // Validate destination
        if self.destination_base.to_str().map_or(true, |s| s.is_empty()) {
            return Err("No destination specified".to_string());
        }
        // Validate LZMA2 settings
        if !self.lzma2.is_valid_dictionary() {
            return Err(format!(
                "Invalid LZMA2 dictionary size: {}. Must be between 2^16 and 2^27",
                self.lzma2.dictionary_size
            ));
        }
        if !self.lzma2.is_valid_word_size() {
            return Err(format!(
                "Invalid LZMA2 word size: {}. Must be between 5 and 273",
                self.lzma2.word_size
            ));
        }
        Ok(())
    }

    /// Set archive format
    pub fn set_format(&mut self, format: ArchiveFormat) {
        self.format = format;
        // Reset LZMA2 settings when switching away from 7z
        if format != ArchiveFormat::SevenZ {
            self.show_advanced = false;
        }
    }

    /// Set compression level
    pub fn set_level(&mut self, level: u32) {
        self.level = CompressionLevel::new(level);
    }

    /// Set password
    pub fn set_password(&mut self, password: Option<String>) {
        // Auto-enable encryption when password is set and non-empty
        let should_encrypt = password.as_ref().map_or(false, |p| !p.is_empty());
        // Treat empty string as None
        self.password = password.filter(|p| !p.is_empty());
        if should_encrypt {
            self.encrypt = true;
        }
    }

    /// Toggle encryption
    pub fn set_encrypt(&mut self, encrypt: bool) {
        self.encrypt = encrypt;
        if !encrypt {
            self.password = None;
        }
    }

    /// Toggle advanced LZMA2 options
    pub fn toggle_advanced(&mut self) {
        self.show_advanced = !self.show_advanced;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ArchiveFormat tests
    #[test]
    fn test_archive_format_extension() {
        assert_eq!(ArchiveFormat::Zip.extension(), ".zip");
        assert_eq!(ArchiveFormat::SevenZ.extension(), ".7z");
        assert_eq!(ArchiveFormat::TarGz.extension(), ".tar.gz");
    }

    #[test]
    fn test_archive_format_all() {
        let formats = ArchiveFormat::all();
        assert_eq!(formats.len(), 3);
        assert!(formats.contains(&ArchiveFormat::Zip));
        assert!(formats.contains(&ArchiveFormat::SevenZ));
        assert!(formats.contains(&ArchiveFormat::TarGz));
    }

    // CompressionLevel tests
    #[test]
    fn test_compression_level_new() {
        assert_eq!(CompressionLevel::new(5).0, 5);
        assert_eq!(CompressionLevel::new(0).0, 0);
        assert_eq!(CompressionLevel::new(9).0, 9);
    }

    #[test]
    fn test_compression_level_clamp() {
        assert_eq!(CompressionLevel::new(15).0, 9);
        assert_eq!(CompressionLevel::new(0).0, 0);
        assert_eq!(CompressionLevel::new(100).0, 9);
    }

    #[test]
    fn test_compression_level_description() {
        assert_eq!(CompressionLevel::new(0).description(), "Store (no compression)");
        assert_eq!(CompressionLevel::new(1).description(), "Fastest");
        assert_eq!(CompressionLevel::new(5).description(), "Default");
        assert_eq!(CompressionLevel::new(9).description(), "Maximum");
    }

    #[test]
    fn test_compression_level_constants() {
        assert_eq!(CompressionLevel::MIN, 0);
        assert_eq!(CompressionLevel::MAX, 9);
        assert_eq!(CompressionLevel::STORE, 0);
        assert_eq!(CompressionLevel::FAST, 1);
        assert_eq!(CompressionLevel::DEFAULT, 5);
        assert_eq!(CompressionLevel::MAXIMUM, 9);
    }

    // Lzma2Settings tests
    #[test]
    fn test_lzma2_settings_default() {
        let settings = Lzma2Settings::default();
        assert_eq!(settings.dictionary_size, 16);
        assert_eq!(settings.word_size, 32);
        assert_eq!(settings.cpu_threads, 0);
    }

    #[test]
    fn test_lzma2_settings_valid_dictionary() {
        let mut settings = Lzma2Settings::default();
        settings.dictionary_size = 16;
        assert!(settings.is_valid_dictionary());
        settings.dictionary_size = 27;
        assert!(settings.is_valid_dictionary());
        settings.dictionary_size = 15;
        assert!(!settings.is_valid_dictionary());
        settings.dictionary_size = 28;
        assert!(!settings.is_valid_dictionary());
    }

    #[test]
    fn test_lzma2_settings_valid_word_size() {
        let mut settings = Lzma2Settings::default();
        settings.word_size = 32;
        assert!(settings.is_valid_word_size());
        settings.word_size = 5;
        assert!(settings.is_valid_word_size());
        settings.word_size = 273;
        assert!(settings.is_valid_word_size());
        settings.word_size = 4;
        assert!(!settings.is_valid_word_size());
        settings.word_size = 274;
        assert!(!settings.is_valid_word_size());
    }

    #[test]
    fn test_lzma2_settings_dictionary_bytes() {
        let mut settings = Lzma2Settings::default();
        settings.dictionary_size = 16;
        assert_eq!(settings.dictionary_bytes(), 65536); // 2^16 = 64KB
        settings.dictionary_size = 20;
        assert_eq!(settings.dictionary_bytes(), 1048576); // 2^20 = 1MB
        settings.dictionary_size = 27;
        assert_eq!(settings.dictionary_bytes(), 134217728); // 2^27 = 128MB
    }

    // VolumeSize tests
    #[test]
    fn test_volume_size_bytes() {
        assert_eq!(VolumeSize::None.bytes(), None);
        assert!(VolumeSize::Cdrom.bytes().is_some());
        assert!(VolumeSize::Dvd.bytes().is_some());
        assert!(VolumeSize::Fat32.bytes().is_some());
        assert_eq!(VolumeSize::Custom(1024).bytes(), Some(1024));
    }

    #[test]
    fn test_volume_size_description() {
        assert_eq!(VolumeSize::None.description(), "No splitting");
        assert_eq!(VolumeSize::Cdrom.description(), "CD (700 MB)");
        assert_eq!(VolumeSize::Dvd.description(), "DVD (4.7 GB)");
        assert_eq!(VolumeSize::Fat32.description(), "FAT32 (4 GB)");
        assert_eq!(VolumeSize::Custom(1024).description(), "Custom size");
    }

    // CreateArchiveState tests
    #[test]
    fn test_create_archive_state_default() {
        let state = CreateArchiveState::default();
        assert_eq!(state.format, ArchiveFormat::Zip);
        assert!(state.source_paths.is_empty());
        assert_eq!(state.level.0, CompressionLevel::DEFAULT);
        assert!(!state.encrypt);
        assert!(state.password.is_none());
        assert_eq!(state.volume, VolumeSize::None);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_create_archive_state_new() {
        let sources = vec![PathBuf::from("C:\\test\\file1.txt")];
        let dest = PathBuf::from("C:\\test\\archive");
        let state = CreateArchiveState::new(sources.clone(), dest.clone());
        assert_eq!(state.source_paths, sources);
        assert_eq!(state.destination_base, dest);
    }

    #[test]
    fn test_create_archive_state_destination_path() {
        let mut state = CreateArchiveState::default();
        state.destination_base = PathBuf::from("C:\\test\\archive");
        
        state.format = ArchiveFormat::Zip;
        assert_eq!(state.destination_path(), PathBuf::from("C:\\test\\archive.zip"));
        
        state.format = ArchiveFormat::SevenZ;
        assert_eq!(state.destination_path(), PathBuf::from("C:\\test\\archive.7z"));
        
        state.format = ArchiveFormat::TarGz;
        assert_eq!(state.destination_path(), PathBuf::from("C:\\test\\archive.tar.gz"));
    }

    #[test]
    fn test_create_archive_state_validate_empty_sources() {
        let state = CreateArchiveState::default();
        let result = state.validate();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "No files selected for archiving");
    }

    #[test]
    fn test_create_archive_state_validate_empty_source_paths() {
        let state = CreateArchiveState::default();
        let result = state.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No files selected"));
    }

    #[test]
    fn test_create_archive_state_validate_empty_destination() {
        let mut state = CreateArchiveState::default();
        state.source_paths.push(PathBuf::from("C:\\test\\file.txt"));
        // destination_base is empty by default
        let result = state.validate();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "No destination specified");
    }

    #[test]
    fn test_create_archive_state_validate_encryption_without_password() {
        let mut state = CreateArchiveState::default();
        state.source_paths.push(PathBuf::from("C:\\test\\file.txt"));
        state.destination_base = PathBuf::from("C:\\test\\archive");
        state.encrypt = true;
        let result = state.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Password is required"));
    }

    #[test]
    fn test_create_archive_state_validate_invalid_lzma2_dictionary() {
        let mut state = CreateArchiveState::default();
        state.source_paths.push(PathBuf::from("C:\\test\\file.txt"));
        state.destination_base = PathBuf::from("C:\\test\\archive");
        state.lzma2.dictionary_size = 15; // Invalid
        let result = state.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid LZMA2 dictionary size"));
    }

    #[test]
    fn test_create_archive_state_validate_invalid_lzma2_word_size() {
        let mut state = CreateArchiveState::default();
        state.source_paths.push(PathBuf::from("C:\\test\\file.txt"));
        state.destination_base = PathBuf::from("C:\\test\\archive");
        state.lzma2.word_size = 4; // Invalid
        let result = state.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid LZMA2 word size"));
    }

    #[test]
    fn test_create_archive_state_validate_success() {
        let mut state = CreateArchiveState::default();
        state.source_paths.push(PathBuf::from("C:\\test\\file.txt"));
        state.destination_base = PathBuf::from("C:\\test\\archive");
        let result = state.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_archive_state_set_format() {
        let mut state = CreateArchiveState::default();
        state.show_advanced = true;
        
        state.set_format(ArchiveFormat::SevenZ);
        assert_eq!(state.format, ArchiveFormat::SevenZ);
        
        // Advanced should remain when switching to 7z
        state.set_format(ArchiveFormat::Zip);
        // Advanced should reset when switching away from 7z
        assert!(!state.show_advanced);
    }

    #[test]
    fn test_create_archive_state_set_level() {
        let mut state = CreateArchiveState::default();
        state.set_level(3);
        assert_eq!(state.level.0, 3);
        state.set_level(15);
        assert_eq!(state.level.0, 9); // Clamped
        state.set_level(0);
        assert_eq!(state.level.0, 0);
    }

    #[test]
    fn test_create_archive_state_set_password() {
        let mut state = CreateArchiveState::default();
        state.set_password(Some("secret".to_string()));
        assert_eq!(state.password, Some("secret".to_string()));
        assert!(state.encrypt); // Auto-enabled
        
        state.set_password(Some("".to_string()));
        assert!(state.password.is_none());
    }

    #[test]
    fn test_create_archive_state_set_encrypt() {
        let mut state = CreateArchiveState::default();
        state.password = Some("secret".to_string());
        
        state.set_encrypt(false);
        assert!(!state.encrypt);
        assert!(state.password.is_none()); // Password cleared
        
        state.password = Some("secret".to_string());
        state.set_encrypt(true);
        assert!(state.encrypt);
        assert_eq!(state.password, Some("secret".to_string()));
    }

    #[test]
    fn test_create_archive_state_toggle_advanced() {
        let mut state = CreateArchiveState::default();
        assert!(!state.show_advanced);
        state.toggle_advanced();
        assert!(state.show_advanced);
        state.toggle_advanced();
        assert!(!state.show_advanced);
    }
}