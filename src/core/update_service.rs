//! # App Update Service
//!
//! Checks for application updates by querying GitHub releases.
//! Supports sideload/portable detection and automatic update checking.
//!
//! ## Architecture
//!
//! - `ReleaseInfo` — parsed information about a GitHub release
//! - `AppUpdateService` — fetches latest release info, compares versions
//! - Uses `curl` CLI to query GitHub API (no new HTTP dependencies)

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

/// Information about a GitHub release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    /// Tag name like "v0.1.0"
    pub tag_name: String,
    /// Human-readable release name
    pub name: String,
    /// Body / release notes
    pub body: String,
    /// Whether this is a pre-release
    pub prerelease: bool,
    /// Published date (ISO 8601)
    pub published_at: String,
    /// Download URL for the asset
    pub html_url: String,
}

impl ReleaseInfo {
    /// Parse version from tag name (strip leading "v" or "V").
    pub fn version(&self) -> &str {
        self.tag_name
            .strip_prefix('v')
            .or_else(|| self.tag_name.strip_prefix('V'))
            .unwrap_or(&self.tag_name)
    }

    /// Compare two semantic versions (simple dot-separated comparison).
    pub fn compare_versions(a: &str, b: &str) -> Ordering {
        let a_parts: Vec<u64> = a.split('.').filter_map(|s| s.parse::<u64>().ok()).collect();
        let b_parts: Vec<u64> = b.split('.').filter_map(|s| s.parse::<u64>().ok()).collect();

        for i in 0..a_parts.len().max(b_parts.len()) {
            let a_val = a_parts.get(i).copied().unwrap_or(0);
            let b_val = b_parts.get(i).copied().unwrap_or(0);
            match a_val.cmp(&b_val) {
                Ordering::Less => return Ordering::Less,
                Ordering::Greater => return Ordering::Greater,
                Ordering::Equal => continue,
            }
        }
        Ordering::Equal
    }
}

impl fmt::Display for ReleaseInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}){}",
            self.name,
            self.tag_name,
            if self.prerelease {
                " [pre-release]"
            } else {
                ""
            }
        )
    }
}

/// Service for checking application updates.
#[derive(Debug, Clone)]
pub struct AppUpdateService {
    /// GitHub owner/repo (e.g., "user/files-rs")
    repo: String,
    /// Current application version
    current_version: String,
}

impl AppUpdateService {
    /// Create a new update service for the given GitHub repository.
    pub fn new(repo: &str, current_version: &str) -> Self {
        Self {
            repo: repo.to_string(),
            current_version: current_version.to_string(),
        }
    }

    /// Get the GitHub API URL for latest release.
    pub fn api_url(&self) -> String {
        format!("https://api.github.com/repos/{}/releases/latest", self.repo)
    }

    /// Get the releases API URL (for listing all releases).
    pub fn api_url_all(&self) -> String {
        format!("https://api.github.com/repos/{}/releases", self.repo)
    }

    /// Fetch the latest release using curl.
    /// Returns `None` if the request fails or no release is found.
    pub fn check_latest(&self) -> Result<Option<ReleaseInfo>, String> {
        let url = self.api_url();
        let output = Self::curl_get(&url)?;
        Self::parse_release_json(&output)
    }

    /// Fetch all releases (for sideload/pre-release checking).
    pub fn check_all_releases(&self) -> Result<Vec<ReleaseInfo>, String> {
        let url = self.api_url_all();
        let output = Self::curl_get(&url)?;
        Self::parse_releases_json(&output)
    }

    /// Check if an update is available.
    pub fn update_available(&self, latest: &ReleaseInfo) -> bool {
        let current = self.current_version.as_str();
        let latest_ver = latest.version();
        ReleaseInfo::compare_versions(latest_ver, current) == Ordering::Greater
    }

    /// Execute a curl GET request and return the response body.
    fn curl_get(url: &str) -> Result<String, String> {
        let output = std::process::Command::new("curl")
            .args([
                "-sS",
                "-H",
                "Accept: application/vnd.github+json",
                "-H",
                "User-Agent: files-rs/0.1.0",
                url,
            ])
            .output()
            .map_err(|e| format!("Failed to execute curl: {}", e))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(stdout)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(format!("curl failed: {}", stderr))
        }
    }

    /// Parse a single release from JSON (GitHub API /releases/latest).
    fn parse_release_json(json: &str) -> Result<Option<ReleaseInfo>, String> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("Failed to parse JSON: {}", e))?;

        // Check for API error
        if let Some(msg) = value.get("message").and_then(|m| m.as_str()) {
            return Err(format!("GitHub API error: {}", msg));
        }

        let tag_name = value
            .get("tag_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing tag_name".to_string())?;

        Ok(Some(ReleaseInfo {
            tag_name: tag_name.to_string(),
            name: value
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            body: value
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            prerelease: value
                .get("prerelease")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            published_at: value
                .get("published_at")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            html_url: value
                .get("html_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        }))
    }

    /// Parse a list of releases from JSON (GitHub API /releases).
    fn parse_releases_json(json: &str) -> Result<Vec<ReleaseInfo>, String> {
        let values: Vec<serde_json::Value> = serde_json::from_str(json)
            .map_err(|e| format!("Failed to parse releases JSON: {}", e))?;

        let mut releases = Vec::new();
        for value in &values {
            if let Some(tag_name) = value.get("tag_name").and_then(|v| v.as_str()) {
                releases.push(ReleaseInfo {
                    tag_name: tag_name.to_string(),
                    name: value
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    body: value
                        .get("body")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    prerelease: value
                        .get("prerelease")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    published_at: value
                        .get("published_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    html_url: value
                        .get("html_url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                });
            }
        }
        Ok(releases)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // ReleaseInfo tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_version_strips_v_prefix() {
        let r = ReleaseInfo {
            tag_name: "v0.2.0".into(),
            name: String::new(),
            body: String::new(),
            prerelease: false,
            published_at: String::new(),
            html_url: String::new(),
        };
        assert_eq!(r.version(), "0.2.0");
    }

    #[test]
    fn test_version_strips_V_prefix() {
        let r = ReleaseInfo {
            tag_name: "V1.0.0".into(),
            ..Default::default()
        };
        assert_eq!(r.version(), "1.0.0");
    }

    #[test]
    fn test_version_no_prefix() {
        let r = ReleaseInfo {
            tag_name: "1.0.0".into(),
            ..Default::default()
        };
        assert_eq!(r.version(), "1.0.0");
    }

    #[test]
    fn test_display_release() {
        let r = ReleaseInfo {
            tag_name: "v0.2.0".into(),
            name: "Version 0.2.0".into(),
            prerelease: false,
            ..Default::default()
        };
        let display = format!("{}", r);
        assert!(display.contains("Version 0.2.0"));
        assert!(display.contains("v0.2.0"));
    }

    #[test]
    fn test_display_prerelease() {
        let r = ReleaseInfo {
            tag_name: "v0.3.0-beta".into(),
            name: "Beta".into(),
            prerelease: true,
            ..Default::default()
        };
        let display = format!("{}", r);
        assert!(display.contains("pre-release"));
    }

    // -----------------------------------------------------------------------
    // Version comparison tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_compare_versions_equal() {
        assert_eq!(
            ReleaseInfo::compare_versions("0.1.0", "0.1.0"),
            Ordering::Equal
        );
    }

    #[test]
    fn test_compare_versions_newer() {
        assert_eq!(
            ReleaseInfo::compare_versions("0.2.0", "0.1.0"),
            Ordering::Greater
        );
        assert_eq!(
            ReleaseInfo::compare_versions("1.0.0", "0.9.9"),
            Ordering::Greater
        );
        assert_eq!(
            ReleaseInfo::compare_versions("0.1.10", "0.1.9"),
            Ordering::Greater
        );
    }

    #[test]
    fn test_compare_versions_older() {
        assert_eq!(
            ReleaseInfo::compare_versions("0.1.0", "0.2.0"),
            Ordering::Less
        );
        assert_eq!(
            ReleaseInfo::compare_versions("0.9.9", "1.0.0"),
            Ordering::Less
        );
    }

    #[test]
    fn test_compare_versions_different_lengths() {
        assert_eq!(
            ReleaseInfo::compare_versions("0.1", "0.1.0"),
            Ordering::Equal
        );
        assert_eq!(
            ReleaseInfo::compare_versions("0.2", "0.1.9"),
            Ordering::Greater
        );
    }

    #[test]
    fn test_compare_versions_non_numeric_fallback() {
        // Non-numeric parts are treated as 0
        assert_eq!(
            ReleaseInfo::compare_versions("0.1.0-beta", "0.1.0"),
            Ordering::Equal
        );
    }

    // -----------------------------------------------------------------------
    // AppUpdateService tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_api_url() {
        let svc = AppUpdateService::new("user/files-rs", "0.1.0");
        assert_eq!(
            svc.api_url(),
            "https://api.github.com/repos/user/files-rs/releases/latest"
        );
    }

    #[test]
    fn test_update_available_true() {
        let svc = AppUpdateService::new("user/files-rs", "0.1.0");
        let latest = ReleaseInfo {
            tag_name: "v0.2.0".into(),
            ..Default::default()
        };
        assert!(svc.update_available(&latest));
    }

    #[test]
    fn test_update_available_false_same_version() {
        let svc = AppUpdateService::new("user/files-rs", "0.1.0");
        let latest = ReleaseInfo {
            tag_name: "v0.1.0".into(),
            ..Default::default()
        };
        assert!(!svc.update_available(&latest));
    }

    #[test]
    fn test_update_available_false_older() {
        let svc = AppUpdateService::new("user/files-rs", "0.2.0");
        let latest = ReleaseInfo {
            tag_name: "v0.1.0".into(),
            ..Default::default()
        };
        assert!(!svc.update_available(&latest));
    }

    // -----------------------------------------------------------------------
    // JSON parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_release_json_success() {
        let json = r#"{
            "tag_name": "v0.2.0",
            "name": "Version 0.2.0",
            "body": "Bug fixes and improvements",
            "prerelease": false,
            "published_at": "2026-01-15T10:00:00Z",
            "html_url": "https://github.com/user/files-rs/releases/tag/v0.2.0"
        }"#;

        let result = AppUpdateService::parse_release_json(json).unwrap();
        assert!(result.is_some());
        let release = result.unwrap();
        assert_eq!(release.tag_name, "v0.2.0");
        assert_eq!(release.name, "Version 0.2.0");
        assert!(!release.prerelease);
        assert_eq!(
            release.html_url,
            "https://github.com/user/files-rs/releases/tag/v0.2.0"
        );
    }

    #[test]
    fn test_parse_release_json_api_error() {
        let json = r#"{"message": "Not Found", "documentation_url": "..."}"#;
        let result = AppUpdateService::parse_release_json(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Not Found"));
    }

    #[test]
    fn test_parse_release_json_invalid() {
        let result = AppUpdateService::parse_release_json("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_releases_json_empty() {
        let result = AppUpdateService::parse_releases_json("[]").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_releases_json_multiple() {
        let json = r#"[
            {"tag_name": "v0.2.0", "name": "v0.2.0", "body": "", "prerelease": false, "published_at": "", "html_url": ""},
            {"tag_name": "v0.1.0", "name": "v0.1.0", "body": "", "prerelease": false, "published_at": "", "html_url": ""}
        ]"#;
        let result = AppUpdateService::parse_releases_json(json).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].tag_name, "v0.2.0");
        assert_eq!(result[1].tag_name, "v0.1.0");
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_release_info_default() {
        let r = ReleaseInfo::default();
        assert_eq!(r.tag_name, "");
        assert_eq!(r.name, "");
        assert!(r.body.is_empty());
        assert!(!r.prerelease);
    }

    #[test]
    fn test_parse_release_json_minimal() {
        let json = r#"{"tag_name": "v1.0.0"}"#;
        let result = AppUpdateService::parse_release_json(json).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().tag_name, "v1.0.0");
    }

    #[test]
    fn test_compare_versions_vs_no_v() {
        assert_eq!(
            ReleaseInfo::compare_versions("0.1.0", "v0.1.0"),
            Ordering::Equal
        );
    }

    #[test]
    fn test_curl_get_nonexistent_url() {
        // Should fail gracefully, not panic
        let result = AppUpdateService::curl_get("https://nonexistent.domain.local/test");
        assert!(result.is_err());
    }
}
