//! Credential Dialog Service
//! State machine for CredentialDialog (username/password for network shares).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Credential save options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CredentialSaveOption {
    None,
    Remember,
    RememberPermanently,
}

impl Default for CredentialSaveOption {
    fn default() -> Self {
        Self::None
    }
}

/// State for CredentialDialog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialState {
    /// Target/server URL
    pub server: String,
    /// Share/resource path
    pub share: Option<String>,
    /// Username
    pub username: String,
    /// Password
    pub password: String,
    /// Show password (visible or hidden)
    pub show_password: bool,
    /// Save credential option
    pub save_option: CredentialSaveOption,
    /// Domain (for domain credentials)
    pub domain: Option<String>,
    /// Error message
    pub error: Option<String>,
}

impl Default for CredentialState {
    fn default() -> Self {
        Self {
            server: String::new(),
            share: None,
            username: String::new(),
            password: String::new(),
            show_password: false,
            save_option: CredentialSaveOption::default(),
            domain: None,
            error: None,
        }
    }
}

impl CredentialState {
    /// Create a new credential state for a server
    pub fn new(server: String) -> Self {
        Self {
            server,
            ..Default::default()
        }
    }

    /// Create for a share (server + share path)
    pub fn for_share(server: String, share: String) -> Self {
        Self {
            server,
            share: Some(share),
            ..Default::default()
        }
    }

    /// Set the username
    pub fn set_username(&mut self, username: String) {
        self.username = username;
    }

    /// Set the password
    pub fn set_password(&mut self, password: String) {
        self.password = password;
    }

    /// Toggle password visibility
    pub fn toggle_show_password(&mut self) {
        self.show_password = !self.show_password;
    }

    /// Set save option
    pub fn set_save_option(&mut self, option: CredentialSaveOption) {
        self.save_option = option;
    }

    /// Set domain
    pub fn set_domain(&mut self, domain: Option<String>) {
        self.domain = domain;
    }

    /// Clear error
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Set error message
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    /// Toggle save option (cycle through None -> Remember -> RememberPermanently)
    pub fn cycle_save_option(&mut self) {
        self.save_option = match self.save_option {
            CredentialSaveOption::None => CredentialSaveOption::Remember,
            CredentialSaveOption::Remember => CredentialSaveOption::RememberPermanently,
            CredentialSaveOption::RememberPermanently => CredentialSaveOption::None,
        };
    }

    /// Get full username with domain (domain\username format)
    pub fn full_username(&self) -> String {
        if let Some(ref domain) = self.domain {
            format!("{}\\{}", domain, self.username)
        } else {
            self.username.clone()
        }
    }

    /// Get the target (server + share)
    pub fn target(&self) -> String {
        if let Some(ref share) = self.share {
            format!("{}\\{}", self.server, share)
        } else {
            self.server.clone()
        }
    }

    /// Validate the credentials
    pub fn validate(&self) -> Result<(), String> {
        if self.server.is_empty() {
            return Err("Server address is required".to_string());
        }
        if self.username.is_empty() {
            return Err("Username is required".to_string());
        }
        // Password can be empty for some cases (e.g., anonymous shares)
        Ok(())
    }

    /// Check if we have credentials to submit
    pub fn has_credentials(&self) -> bool {
        !self.username.is_empty()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_save_option_default() {
        assert_eq!(CredentialSaveOption::default(), CredentialSaveOption::None);
    }

    #[test]
    fn test_credential_state_default() {
        let state = CredentialState::default();
        assert!(state.server.is_empty());
        assert!(state.share.is_none());
        assert!(state.username.is_empty());
        assert!(state.password.is_empty());
        assert!(!state.show_password);
        assert_eq!(state.save_option, CredentialSaveOption::None);
        assert!(state.domain.is_none());
        assert!(state.error.is_none());
    }

    #[test]
    fn test_credential_state_new() {
        let state = CredentialState::new("server.example.com".to_string());
        assert_eq!(state.server, "server.example.com");
        assert!(state.share.is_none());
    }

    #[test]
    fn test_credential_state_for_share() {
        let state = CredentialState::for_share(
            "server.example.com".to_string(),
            "shared_folder".to_string(),
        );
        assert_eq!(state.server, "server.example.com");
        assert_eq!(state.share, Some("shared_folder".to_string()));
    }

    #[test]
    fn test_credential_state_set_username() {
        let mut state = CredentialState::default();
        state.set_username("user1".to_string());
        assert_eq!(state.username, "user1");
    }

    #[test]
    fn test_credential_state_set_password() {
        let mut state = CredentialState::default();
        state.set_password("secret".to_string());
        assert_eq!(state.password, "secret");
    }

    #[test]
    fn test_credential_state_toggle_show_password() {
        let mut state = CredentialState::default();
        assert!(!state.show_password);
        state.toggle_show_password();
        assert!(state.show_password);
        state.toggle_show_password();
        assert!(!state.show_password);
    }

    #[test]
    fn test_credential_state_set_save_option() {
        let mut state = CredentialState::default();
        state.set_save_option(CredentialSaveOption::Remember);
        assert_eq!(state.save_option, CredentialSaveOption::Remember);
        state.set_save_option(CredentialSaveOption::RememberPermanently);
        assert_eq!(state.save_option, CredentialSaveOption::RememberPermanently);
    }

    #[test]
    fn test_credential_state_set_domain() {
        let mut state = CredentialState::default();
        state.set_domain(Some("DOMAIN".to_string()));
        assert_eq!(state.domain, Some("DOMAIN".to_string()));
        state.set_domain(None);
        assert!(state.domain.is_none());
    }

    #[test]
    fn test_credential_state_clear_error() {
        let mut state = CredentialState::default();
        state.set_error("Test error".to_string());
        assert!(state.error.is_some());
        state.clear_error();
        assert!(state.error.is_none());
    }

    #[test]
    fn test_credential_state_cycle_save_option() {
        let mut state = CredentialState::default();
        
        assert_eq!(state.save_option, CredentialSaveOption::None);
        state.cycle_save_option();
        assert_eq!(state.save_option, CredentialSaveOption::Remember);
        state.cycle_save_option();
        assert_eq!(state.save_option, CredentialSaveOption::RememberPermanently);
        state.cycle_save_option();
        assert_eq!(state.save_option, CredentialSaveOption::None);
    }

    #[test]
    fn test_credential_state_full_username() {
        let mut state = CredentialState::default();
        state.username = "user1".to_string();
        assert_eq!(state.full_username(), "user1");
        
        state.set_domain(Some("DOMAIN".to_string()));
        assert_eq!(state.full_username(), "DOMAIN\\user1");
    }

    #[test]
    fn test_credential_state_target() {
        let mut state = CredentialState::default();
        state.server = "server.example.com".to_string();
        assert_eq!(state.target(), "server.example.com");
        
        state.share = Some("shared".to_string());
        assert_eq!(state.target(), "server.example.com\\shared");
    }

    #[test]
    fn test_credential_state_validate_empty_server() {
        let state = CredentialState::default();
        let result = state.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Server address"));
    }

    #[test]
    fn test_credential_state_validate_empty_username() {
        let mut state = CredentialState::default();
        state.server = "server.example.com".to_string();
        let result = state.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Username is required"));
    }

    #[test]
    fn test_credential_state_validate_success() {
        let mut state = CredentialState::default();
        state.server = "server.example.com".to_string();
        state.username = "user1".to_string();
        let result = state.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_credential_state_has_credentials() {
        let mut state = CredentialState::default();
        assert!(!state.has_credentials());
        
        state.username = "user1".to_string();
        assert!(state.has_credentials());
    }
}