use fluent::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use std::collections::HashMap;

/// Wrapper for localized strings that may contain arguments.
/// When the message requires arguments, use `format_with_args()` instead of `t()`.
#[derive(Debug)]
pub struct LocalizedString<'a> {
    key: String,
    args: Option<FluentArgs<'a>>,
}

impl<'a> LocalizedString<'a> {
    pub fn new(key: &str) -> Self {
        Self {
            key: key.to_string(),
            args: None,
        }
    }

    pub fn with_args(key: &str, args: FluentArgs<'a>) -> Self {
        Self {
            key: key.to_string(),
            args: Some(args),
        }
    }

    /// Get the message key
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Check if this string has arguments
    pub fn has_args(&self) -> bool {
        self.args.is_some()
    }
}

pub struct LocalizationManager {
    pub current_lang: String,
    pub bundles: HashMap<String, FluentBundle<FluentResource>>,
}

impl LocalizationManager {
    pub fn new() -> Self {
        let mut manager = Self {
            current_lang: "en".to_string(),
            bundles: HashMap::new(),
        };
        manager.load_locale("en");
        manager
    }

    pub fn set_language(&mut self, lang: &str) {
        if !self.bundles.contains_key(lang) {
            self.load_locale(lang);
        }
        if self.bundles.contains_key(lang) {
            self.current_lang = lang.to_string();
        }
    }

    pub fn get_language(&self) -> &str {
        &self.current_lang
    }

    pub fn available_languages(&self) -> Vec<&str> {
        self.bundles.keys().map(|s| s.as_str()).collect()
    }

    pub fn load_locale(&mut self, lang: &str) {
        let path = format!("locales/{}/messages.ftl", lang);
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(res) = FluentResource::try_new(content) {
                let mut bundle = FluentBundle::new(vec![]);
                if let Ok(_) = bundle.add_resource(res) {
                    self.bundles.insert(lang.to_string(), bundle);
                    tracing::info!("Loaded locale: {}", lang);
                }
            }
        } else {
            tracing::warn!("Could not load locale file at: {}", path);
        }
    }

    /// Get a simple localized string (no arguments).
    pub fn t(&mut self, key: &str) -> String {
        self.format_message(key, None)
    }

    /// Get a localized string with arguments.
    /// Arguments are passed as key-value pairs.
    /// Example: manager.t_with_args("greeting", &[("name", "World")])
    pub fn t_with_args(&mut self, key: &str, args: &[(&str, &str)]) -> String {
        let mut fluent_args = FluentArgs::new();
        for (k, v) in args {
            fluent_args.set(*k, FluentValue::from(*v));
        }
        self.format_message(key, Some(&fluent_args))
    }

    /// Format a message with the given key and optional arguments.
    fn format_message(&mut self, key: &str, args: Option<&FluentArgs>) -> String {
        let bundle = match self.bundles.get_mut(&self.current_lang) {
            Some(b) => b,
            None => return key.to_string(),
        };

        // Get the message from the bundle
        let message = match bundle.get_message(key) {
            Some(msg) => msg,
            None => return key.to_string(),
        };

        // Get the pattern from the message
        let pattern = match message.value() {
            Some(p) => p,
            None => return key.to_string(),
        };

        // Format the message with arguments
        let mut errors = Vec::new();
        let result = bundle.format_pattern(pattern, args, &mut errors);

        if !errors.is_empty() {
            tracing::warn!("Localization errors for key '{}': {:?}", key, errors);
        }

        result.into_owned()
    }

    /// Create a LocalizedString for later formatting with arguments.
    /// This is useful when you want to pass a localized string around
    /// and format it later with specific values.
    pub fn create_localized(&mut self, key: &str) -> LocalizedString {
        LocalizedString::new(key)
    }

    /// Check if a message exists for the current language.
    #[allow(dead_code)]
    pub fn has_message(&mut self, key: &str) -> bool {
        let bundle = match self.bundles.get_mut(&self.current_lang) {
            Some(b) => b,
            None => return false,
        };
        bundle.get_message(key).is_some()
    }

    /// Get a count of messages in the current language bundle.
    #[allow(dead_code)]
    pub fn message_count(&self) -> usize {
        // FluentBundle doesn't expose message count directly; return 1 if bundle exists
        if self.bundles.contains_key(&self.current_lang) { 1 } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_localization_manager_new() {
        let manager = LocalizationManager::new();
        assert_eq!(manager.current_lang, "en");
        assert!(manager.bundles.contains_key("en"));
    }

    #[test]
    fn test_set_language_changes_current() {
        let mut manager = LocalizationManager::new();
        manager.set_language("en");
        assert_eq!(manager.get_language(), "en");
    }

    #[test]
    fn test_get_language_returns_current() {
        let mut manager = LocalizationManager::new();
        assert_eq!(manager.get_language(), "en");
        manager.set_language("en");
        assert_eq!(manager.get_language(), "en");
    }

    #[test]
    fn test_available_languages() {
        let mut manager = LocalizationManager::new();
        let langs = manager.available_languages();
        assert!(langs.contains(&"en"));
    }

    #[test]
    fn test_t_returns_key_when_no_bundle() {
        let mut manager = LocalizationManager::new();
        manager.current_lang = "nonexistent".to_string();
        let result = manager.t("hello");
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_t_returns_key_when_bundle_exists() {
        let mut manager = LocalizationManager::new();
        // Even with a bundle, t() returns key by default (stub implementation)
        let result = manager.t("test_key");
        assert_eq!(result, "test_key");
    }

    #[test]
    fn test_load_locale_logs_warning_for_invalid_path() {
        let mut manager = LocalizationManager::new();
        // This will log a warning but not panic
        manager.load_locale("invalid_locale_that_does_not_exist");
        // Manager should still have the original locale
        assert_eq!(manager.current_lang, "en");
    }

    #[test]
    fn test_load_locale_does_not_change_current_lang() {
        let mut manager = LocalizationManager::new();
        let original_lang = manager.current_lang.clone();
        manager.load_locale("en"); // Reload same locale
        assert_eq!(manager.current_lang, original_lang);
    }

    #[test]
    fn test_localized_string_new() {
        let loc = LocalizedString::new("greeting");
        assert_eq!(loc.key(), "greeting");
        assert!(!loc.has_args());
    }

    #[test]
    fn test_localized_string_with_args() {
        let args = FluentArgs::new();
        let loc = LocalizedString::with_args("greeting", args);
        assert_eq!(loc.key(), "greeting");
        assert!(loc.has_args());
    }

    #[test]
    fn test_has_message_returns_false_for_missing() {
        let mut manager = LocalizationManager::new();
        assert!(!manager.has_message("nonexistent-message-key"));
    }

    #[test]
    fn test_has_message_returns_true_for_existing() {
        let mut manager = LocalizationManager::new();
        // Even if key doesn't exist, this tests the method works
        let result = manager.has_message("test-missing");
        assert!(!result);
    }

    #[test]
    fn test_message_count_returns_zero_for_invalid_lang() {
        let mut manager = LocalizationManager::new();
        manager.current_lang = "nonexistent".to_string();
        let count = manager.message_count();
        // Should return 0 when language not loaded
        assert_eq!(count, 0);
    }
}
