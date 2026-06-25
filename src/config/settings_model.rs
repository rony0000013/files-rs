use crate::config::config::AppConfig;
use gpui::*;
use std::time::Instant;

pub struct SettingsModel {
    pub config: AppConfig,
    /// Last time settings were saved to disk (for debounce)
    last_save: Option<Instant>,
}

/// Minimum interval between settings saves to avoid hammering disk
const SAVE_DEBOUNCE_MS: u64 = 500;

impl SettingsModel {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            last_save: None,
        }
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn pin_folder(
        &mut self,
        path: &crate::core::path_util::FilesPath,
        cx: &mut App,
        entity_id: EntityId,
    ) {
        self.update(
            |config| {
                let path_str = path.to_string_lossy().into_owned();
                if !config.sidebar.pinned_locations.contains(&path_str) {
                    config.sidebar.pinned_locations.push(path_str);
                }
            },
            cx,
            entity_id,
        );
    }

    pub fn unpin_folder(
        &mut self,
        path: &crate::core::path_util::FilesPath,
        cx: &mut App,
        entity_id: EntityId,
    ) {
        self.update(
            |config| {
                let path_str = path.to_string_lossy().into_owned();
                config.sidebar.pinned_locations.retain(|p| p != &path_str);
            },
            cx,
            entity_id,
        );
    }

    pub fn toggle_hidden_files(&mut self, cx: &mut App, entity_id: EntityId) {
        self.update(
            |config| {
                config.preferences.show_hidden_files = !config.preferences.show_hidden_files;
            },
            cx,
            entity_id,
        );
    }

    pub fn set_font_family(&mut self, family: String, cx: &mut App, entity_id: EntityId) {
        self.update(
            |config| {
                config.appearance.font_family = family;
            },
            cx,
            entity_id,
        );
    }

    pub fn set_font_size(&mut self, size: f64, cx: &mut App, entity_id: EntityId) {
        self.update(
            |config| {
                config.appearance.font_size = size;
            },
            cx,
            entity_id,
        );
    }

    pub fn toggle_mica_alt(&mut self, cx: &mut App, entity_id: EntityId) {
        self.update(
            |config| {
                config.appearance.mica_alt = !config.appearance.mica_alt;
            },
            cx,
            entity_id,
        );
    }

    pub fn toggle_dot_files(&mut self, cx: &mut App, entity_id: EntityId) {
        self.update(
            |config| {
                config.preferences.show_dot_files = !config.preferences.show_dot_files;
            },
            cx,
            entity_id,
        );
    }

    pub fn toggle_file_extensions(&mut self, cx: &mut App, entity_id: EntityId) {
        self.update(
            |config| {
                config.preferences.show_file_extensions = !config.preferences.show_file_extensions;
            },
            cx,
            entity_id,
        );
    }

    pub fn show_hidden_files(&self) -> bool {
        self.config.preferences.show_hidden_files
    }

    pub fn show_dot_files(&self) -> bool {
        self.config.preferences.show_dot_files
    }

    pub fn show_file_extensions(&self) -> bool {
        self.config.preferences.show_file_extensions
    }

    pub fn show_system_files(&self) -> bool {
        self.config.preferences.show_system_files
    }

    pub fn update<F>(&mut self, f: F, cx: &mut App, entity_id: EntityId)
    where
        F: FnOnce(&mut AppConfig),
    {
        f(&mut self.config);
        // Debounced save — only write to disk if enough time has passed
        let should_save = match self.last_save {
            Some(last) => last.elapsed().as_millis() >= SAVE_DEBOUNCE_MS as u128,
            None => true,
        };
        if should_save {
            if let Err(e) = self.config.save() {
                tracing::error!("Failed to save settings: {:?}", e);
            }
            self.last_save = Some(Instant::now());
        }
        cx.notify(entity_id);
    }
}

impl gpui::Render for SettingsModel {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div().child("Settings Model (Internal)")
    }
}
