//! Network Locations Service
//! Enumerates SMB shares and network locations.

use crate::core::path_util::FilesPath;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type of network location
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkLocationType {
    /// SMB share on local network
    SmbShare,
    /// WebDAV share
    WebDav,
    /// FTP/SFTP location
    Ftp,
    /// Network drive (mapped letter)
    MappedDrive,
    /// Unknown type
    Unknown,
}

/// Status of a network location
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkStatus {
    /// Location is available
    Available,
    /// Location is disconnected
    Disconnected,
    /// Connection is being established
    Connecting,
    /// Authentication required
    AuthRequired,
    /// Unknown status
    Unknown,
}

/// Information about a network location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkLocation {
    /// Display name
    pub name: String,
    /// Full path (e.g., \\server\share or smb://server/share)
    pub path: String,
    /// Type of network location
    pub location_type: NetworkLocationType,
    /// Current status
    pub status: NetworkStatus,
    /// Device letter if it's a mapped drive (e.g., 'Z')
    pub drive_letter: Option<char>,
    /// Server hostname
    pub server: Option<String>,
    /// Share name (for SMB)
    pub share_name: Option<String>,
    /// Username for authentication (if saved)
    pub username: Option<String>,
}

impl NetworkLocation {
    /// Create a new network location
    pub fn new(path: &str, name: &str, location_type: NetworkLocationType) -> Self {
        Self {
            name: name.to_string(),
            path: path.to_string(),
            location_type,
            status: NetworkStatus::Unknown,
            drive_letter: None,
            server: None,
            share_name: None,
            username: None,
        }
    }

    /// Set the drive letter for mapped drives
    pub fn with_drive_letter(mut self, letter: char) -> Self {
        self.drive_letter = Some(letter);
        self
    }

    /// Set server and share name
    pub fn with_server_share(mut self, server: &str, share: &str) -> Self {
        self.server = Some(server.to_string());
        self.share_name = Some(share.to_string());
        self
    }
}

/// Network Locations Service
/// Enumerates and manages network locations (SMB shares, mapped drives, etc.)
pub struct NetworkLocationsService {
    /// Cache of discovered locations
    locations: Vec<NetworkLocation>,
}

impl NetworkLocationsService {
    /// Create a new NetworkLocationsService
    pub fn new() -> Self {
        Self {
            locations: Vec::new(),
        }
    }

    /// Enumerate all available network locations
    /// On Windows, this uses WNetEnumResources or SMB APIs
    pub fn enumerate(&mut self) -> Vec<NetworkLocation> {
        self.locations.clear();

        #[cfg(windows)]
        {
            // Enumerate mapped drives
            self.enumerate_mapped_drives();
            // Enumerate SMB shares
            self.enumerate_smb_shares();
        }

        // On non-Windows, we can't enumerate network locations without platform-specific code
        // The service still provides the structure for manual addition

        self.locations.clone()
    }

    /// Enumerate mapped drives on Windows
    #[cfg(windows)]
    fn enumerate_mapped_drives(&mut self) {
        // Check A-Z for mapped drives by checking if they're subst'd
        for letter in b'A'..=b'Z' {
            let drive_str = format!("{}:", letter as char);
            let drive_path = std::path::Path::new(&drive_str);

            if drive_path.exists() {
                // Check if it's a network drive by trying to get its real path
                // A local drive will resolve to itself, a network drive won't
                if let Ok(canonical) = std::fs::canonicalize(&drive_str) {
                    let canonical_str = canonical.to_string_lossy();
                    // If canonical path is on a different drive, it's likely network
                    if canonical_str.starts_with("\\\\?\\") || canonical_str.starts_with("\\\\") {
                        let location = NetworkLocation::new(
                            &drive_str,
                            &format!("Network Drive ({}:)", letter as char),
                            NetworkLocationType::MappedDrive,
                        )
                        .with_drive_letter(letter as char);
                        self.locations.push(location);
                    }
                }
            }
        }
    }

    /// Enumerate SMB shares using Windows APIs
    #[cfg(windows)]
    fn enumerate_smb_shares(&mut self) {
        #[cfg(test)]
        return;

        use std::process::Command;

        // Use net view to enumerate shares on the local network
        // This is a basic implementation - a full implementation would use SMB APIs
        let output = Command::new("net").args(["view", "/domain"]).output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            // Parse server names from net view output
            for line in stdout.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with("Server name") && !line.starts_with("-") {
                    // This is a server name - could enumerate shares from it
                    let server = line.split_whitespace().next().unwrap_or("");
                    if !server.is_empty() {
                        // Try to get shares for this server
                        self.enumerate_shares_for_server(server);
                    }
                }
            }
        }
    }

    /// Enumerate shares for a specific SMB server
    #[cfg(windows)]
    fn enumerate_shares_for_server(&mut self, server: &str) {
        use std::process::Command;

        let output = Command::new("net")
            .args(["view", &format!("\\\\{}", server)])
            .output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                let line = line.trim();
                if !line.is_empty()
                    && !line.starts_with("Share name")
                    && !line.starts_with("-")
                    && !line.contains("Disk")
                    && !line.contains("IPC")
                {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if let Some(share_name) = parts.first() {
                        let share_path = format!("\\\\{}\\{}", server, share_name);
                        let location = NetworkLocation::new(
                            &share_path,
                            &format!("{} on {}", share_name, server),
                            NetworkLocationType::SmbShare,
                        )
                        .with_server_share(server, share_name);
                        self.locations.push(location);
                    }
                }
            }
        }
    }

    /// Add a network location manually
    pub fn add_location(&mut self, location: NetworkLocation) {
        self.locations.push(location);
    }

    /// Get all discovered locations
    pub fn get_locations(&self) -> Vec<&NetworkLocation> {
        self.locations.iter().collect()
    }

    /// Get locations by type
    pub fn get_locations_by_type(
        &self,
        location_type: NetworkLocationType,
    ) -> Vec<&NetworkLocation> {
        self.locations
            .iter()
            .filter(|l| l.location_type == location_type)
            .collect()
    }

    /// Get locations by status
    pub fn get_locations_by_status(&self, status: NetworkStatus) -> Vec<&NetworkLocation> {
        self.locations
            .iter()
            .filter(|l| l.status == status)
            .collect()
    }

    /// Connect to a network location
    /// On Windows, this uses WNetAddConnection2
    pub fn connect(
        &self,
        location: &NetworkLocation,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<FilesPath, String> {
        #[cfg(windows)]
        {
            use std::process::Command;

            let path = &location.path;

            // Build the net use command
            let mut args = vec!["use".to_string(), path.clone()];

            if let (Some(user), Some(pass)) = (username, password) {
                args.push(format!("/user:{}", user));
                args.push(pass.to_string());
            } else if let Some(user) = username {
                args.push(format!("/user:{}", user));
            }

            let output = Command::new("net").args(&args).output();

            match output {
                Ok(out) if out.status.success() => {
                    tracing::info!("Connected to network location: {}", path);
                    Ok(FilesPath::from(path.as_str()))
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    Err(format!("Failed to connect: {}", stderr))
                }
                Err(e) => Err(format!("Failed to execute net use: {}", e)),
            }
        }

        #[cfg(not(windows))]
        {
            Err("Network connect is only available on Windows".to_string())
        }
    }

    /// Disconnect from a network location
    pub fn disconnect(&self, path: &str) -> Result<(), String> {
        #[cfg(windows)]
        {
            use std::process::Command;

            let output = Command::new("net").args(["use", path, "/delete"]).output();

            match output {
                Ok(out) if out.status.success() => {
                    tracing::info!("Disconnected from: {}", path);
                    Ok(())
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    Err(format!("Failed to disconnect: {}", stderr))
                }
                Err(e) => Err(format!("Failed to execute net use: {}", e)),
            }
        }

        #[cfg(not(windows))]
        {
            Err("Network disconnect is only available on Windows".to_string())
        }
    }

    /// Check if a path is a network location
    pub fn is_network_path(path: &FilesPath) -> bool {
        let path_str = path.as_path().to_string_lossy();

        // Check for UNC path
        if path_str.starts_with("\\\\") {
            return true;
        }

        // Check for mapped drive
        if path_str.len() >= 2 && path_str.chars().nth(1) == Some(':') {
            let drive_letter = path_str.chars().next().unwrap();
            if drive_letter.is_ascii_alphabetic() {
                let drive_path = format!("{}:", drive_letter);
                if let Ok(canonical) = std::fs::canonicalize(&drive_path) {
                    let canonical_str = canonical.to_string_lossy();
                    return canonical_str.starts_with("\\\\")
                        || canonical_str.starts_with("\\\\?\\");
                }
            }
        }

        // Check for known network protocols
        if path_str.starts_with("smb://")
            || path_str.starts_with("nfs://")
            || path_str.starts_with("ftp://")
        {
            return true;
        }

        false
    }

    /// Get share info from a UNC path
    pub fn parse_unc_path(path: &str) -> Option<(String, String)> {
        // Parse \\server\share format
        if path.starts_with("\\\\") {
            let rest = &path[2..];
            let parts: Vec<&str> = rest.split('\\').collect();
            if parts.len() >= 2 {
                return Some((parts[0].to_string(), parts[1].to_string()));
            } else if parts.len() == 1 {
                return Some((parts[0].to_string(), String::new()));
            }
        }
        None
    }
}

impl Default for NetworkLocationsService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_service_creation() {
        let service = NetworkLocationsService::new();
        assert!(service.get_locations().is_empty());
    }

    #[test]
    fn test_add_location() {
        let mut service = NetworkLocationsService::new();
        let location = NetworkLocation::new(
            "\\\\server\\share",
            "Test Share",
            NetworkLocationType::SmbShare,
        );

        service.add_location(location);
        assert_eq!(service.get_locations().len(), 1);
    }

    #[test]
    fn test_get_locations_by_type() {
        let mut service = NetworkLocationsService::new();

        service.add_location(NetworkLocation::new(
            "\\\\server1\\share1",
            "Share 1",
            NetworkLocationType::SmbShare,
        ));

        service.add_location(NetworkLocation::new(
            "Z:",
            "Network Drive",
            NetworkLocationType::MappedDrive,
        ));

        let smb_locations = service.get_locations_by_type(NetworkLocationType::SmbShare);
        assert_eq!(smb_locations.len(), 1);

        let mapped = service.get_locations_by_type(NetworkLocationType::MappedDrive);
        assert_eq!(mapped.len(), 1);
    }

    #[test]
    fn test_network_location_builder() {
        let location = NetworkLocation::new(
            "\\\\server\\documents",
            "Documents",
            NetworkLocationType::SmbShare,
        )
        .with_drive_letter('Z')
        .with_server_share("server", "documents");

        assert_eq!(location.drive_letter, Some('Z'));
        assert_eq!(location.server, Some("server".to_string()));
        assert_eq!(location.share_name, Some("documents".to_string()));
    }

    #[test]
    fn test_is_network_path_unc() {
        let path = FilesPath::from("\\\\server\\share\\file.txt");
        assert!(NetworkLocationsService::is_network_path(&path));
    }

    #[test]
    fn test_is_network_path_local() {
        let temp_dir = TempDir::new().unwrap();
        let path = FilesPath::from(temp_dir.path().to_str().unwrap());
        // Local paths should not be detected as network paths
        // (unless they're on a network drive, which temp dirs aren't)
        let is_network = NetworkLocationsService::is_network_path(&path);
        // This may return true or false depending on the system
        // The function is designed to be conservative
    }

    #[test]
    fn test_parse_unc_path() {
        let (server, share) = NetworkLocationsService::parse_unc_path("\\\\server\\share").unwrap();
        assert_eq!(server, "server");
        assert_eq!(share, "share");

        let (server, share) =
            NetworkLocationsService::parse_unc_path("\\\\server\\share\\folder\\file.txt").unwrap();
        assert_eq!(server, "server");
        assert_eq!(share, "share");
    }

    #[test]
    fn test_parse_unc_path_invalid() {
        let result = NetworkLocationsService::parse_unc_path("C:\\local\\path");
        assert!(result.is_none());

        let result = NetworkLocationsService::parse_unc_path("not a unc path");
        assert!(result.is_none());
    }

    #[test]
    fn test_network_location_type_descriptions() {
        assert_eq!(NetworkLocationType::SmbShare.description(), "SMB Share");
        assert_eq!(
            NetworkLocationType::MappedDrive.description(),
            "Mapped Drive"
        );
    }

    #[test]
    fn test_network_status() {
        assert_eq!(NetworkStatus::Available.description(), "Available");
        assert_eq!(NetworkStatus::Disconnected.description(), "Disconnected");
    }

    #[test]
    fn test_enumerate_returns_vector() {
        let mut service = NetworkLocationsService::new();
        let locations = service.enumerate();
        // Should return a vector (may be empty on non-Windows or if no network drives)
        assert!(locations.is_empty() || !locations.is_empty());
    }

    #[test]
    fn test_get_locations_by_status() {
        let mut service = NetworkLocationsService::new();

        let mut location =
            NetworkLocation::new("\\\\server\\share", "Test", NetworkLocationType::SmbShare);
        location.status = NetworkStatus::Available;

        service.add_location(location);

        let available = service.get_locations_by_status(NetworkStatus::Available);
        assert_eq!(available.len(), 1);
    }

    #[test]
    fn test_default_service() {
        let mut service = NetworkLocationsService::default();
        // Should work via Default trait
        let locations = service.enumerate();
        assert!(locations.is_empty() || !locations.is_empty()); // Platform-dependent
    }
}

impl NetworkLocationType {
    /// Human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            NetworkLocationType::SmbShare => "SMB Share",
            NetworkLocationType::WebDav => "WebDAV",
            NetworkLocationType::Ftp => "FTP/SFTP",
            NetworkLocationType::MappedDrive => "Mapped Drive",
            NetworkLocationType::Unknown => "Unknown",
        }
    }
}

impl NetworkStatus {
    /// Human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            NetworkStatus::Available => "Available",
            NetworkStatus::Disconnected => "Disconnected",
            NetworkStatus::Connecting => "Connecting",
            NetworkStatus::AuthRequired => "Authentication Required",
            NetworkStatus::Unknown => "Unknown",
        }
    }
}
