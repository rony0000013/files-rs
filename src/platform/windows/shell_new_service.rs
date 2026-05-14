//! Windows ShellNew File Templates Service
//! Parses Windows Registry to populate the "New" context menu with file templates.

use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(windows)]
use winreg::enums::*;
#[cfg(windows)]
use winreg::RegKey;

/// A file template from the Windows ShellNew registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellNewTemplate {
    /// Display name (e.g., "Text Document", "Bitmap Image")
    pub name: String,
    /// File extension (e.g., ".txt", ".bmp")
    pub extension: String,
    /// Optional template file path
    pub template_path: Option<String>,
    /// Optional icon resource index
    pub icon_index: Option<i32>,
    /// Null file flag (creates zero-byte file if true)
    pub null_file: bool,
}

/// Windows ShellNew Service
/// Reads Windows Registry to enumerate available "New" file templates.
pub struct ShellNewService {
    #[cfg(windows)]
    templates_cache: std::sync::Mutex<Option<Vec<ShellNewTemplate>>>,
}

impl ShellNewService {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            templates_cache: std::sync::Mutex::new(None),
        }
    }

    /// Get all available ShellNew templates from registry
    #[cfg(windows)]
    pub fn get_templates(&self) -> Result<Vec<ShellNewTemplate>> {
        // Check cache first
        {
            let cache = self.templates_cache.lock().unwrap();
            if let Some(ref templates) = *cache {
                return Ok(templates.clone());
            }
        }

        let mut templates = Vec::new();

        // Parse HKEY_CLASSES_ROOT for ShellNew entries
        // Each extension can have a ShellNew subkey with template info
        let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);

        // Get list of all extensions from registry
        if let Ok(extensions_key) = hkcr.open_subkey("") {
            for value_result in extensions_key.enum_values() {
                if let Ok((name, _)) = value_result {
                    // Only process keys that look like extensions
                    if name.starts_with('.') {
                        if let Ok(ext_templates) = self.get_extension_shellnew(&hkcr, &name) {
                            templates.extend(ext_templates);
                        }
                    }
                }
            }
        }

        // Also check known file type associations
        let known_types = [
            ".txt", ".rtf", ".docx", ".xlsx", ".pptx",
            ".bmp", ".jpg", ".png", ".gif", ".ico",
            ".zip", ".7z", ".rar",
            ".html", ".css", ".js", ".json", ".xml",
            ".pdf", ".psd", ".ai",
        ];

        for ext in known_types {
            if let Ok(ext_templates) = self.get_extension_shellnew(&hkcr, ext) {
                for template in ext_templates {
                    if !templates.iter().any(|t: &ShellNewTemplate| t.extension == template.extension) {
                        templates.push(template);
                    }
                }
            }
        }

        // Sort by display name
        templates.sort_by(|a, b| a.name.cmp(&b.name));

        // Cache the result
        {
            let mut cache = self.templates_cache.lock().unwrap();
            *cache = Some(templates.clone());
        }

        tracing::debug!("Loaded {} ShellNew templates", templates.len());
        Ok(templates)
    }

    #[cfg(windows)]
    fn get_extension_shellnew(&self, hkcr: &RegKey, extension: &str) -> Result<Vec<ShellNewTemplate>> {
        use std::io;

        let mut templates = Vec::new();

        // Open the extension key to get the file type description
        let file_type: Option<String> = hkcr.get_value(extension).ok();

        // Open ShellNew subkey
        let shellnew_path = format!("{}\\ShellNew", extension);
        if let Ok(shellnew_key) = hkcr.open_subkey(&shellnew_path) {
            // Determine display name from file type or extension
            let name = file_type
                .or_else(|| {
                    // Convert extension to readable name
                    let ext_clean = extension.trim_start_matches('.');
                    match ext_clean.to_lowercase().as_str() {
                        "txt" => Some("Text Document".to_string()),
                        "rtf" => Some("Rich Text Document".to_string()),
                        "bmp" => Some("Bitmap Image".to_string()),
                        "jpg" => Some("JPEG Image".to_string()),
                        "png" => Some("PNG Image".to_string()),
                        "gif" => Some("GIF Image".to_string()),
                        "html" => Some("HTML Document".to_string()),
                        "css" => Some("Cascading Style Sheet".to_string()),
                        "js" => Some("JavaScript File".to_string()),
                        "json" => Some("JSON File".to_string()),
                        "xml" => Some("XML File".to_string()),
                        "pdf" => Some("PDF Document".to_string()),
                        "zip" => Some("Compressed (zipped) Folder".to_string()),
                        _ => Some(format!("{} File", ext_clean.to_uppercase())),
                    }
                })
                .unwrap_or_else(|| extension.to_string());

            // Read ShellNew values
            let template_path: Option<String> = shellnew_key.get_value("FileName").ok();
            let null_file: bool = shellnew_key.get_value::<u32, _>("NullFile").unwrap_or(0) != 0;
            let icon_index: Option<i32> = shellnew_key.get_value::<u32, _>("IconIndex").ok().map(|v| v as i32);

            // Data value is binary template - skip for now as winreg requires complex handling
            // The template_path or null_file is sufficient for most use cases
            let _data_template: Option<Vec<u8>> = None;

            // Only add if we have a valid template
            if null_file || template_path.is_some() {
                templates.push(ShellNewTemplate {
                    name,
                    extension: extension.to_string(),
                    template_path,
                    icon_index,
                    null_file,
                });
            }
        }

        Ok(templates)
    }

    /// Get templates (non-Windows stub)
    #[cfg(not(windows))]
    pub fn get_templates(&self) -> Result<Vec<ShellNewTemplate>> {
        Ok(Vec::new())
    }

    /// Create a new file from a ShellNew template
    #[cfg(windows)]
    pub async fn create_from_template(&self, parent_dir: &FilesPath, template: &ShellNewTemplate) -> Result<FilesPath> {
        use tokio::fs::File;
        use tokio::io::AsyncWriteExt;

        let file_name = format!("New {}", &template.name.replace(" Document", "").replace(" Image", "").replace(" File", ""));
        let mut file_path = parent_dir.as_path().join(&file_name);
        
        // Ensure unique filename
        let mut counter = 1;
        let stem = file_path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| file_name.clone());
        let ext = file_path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();

        while file_path.exists() {
            file_path = if ext.is_empty() {
                parent_dir.as_path().join(format!("{} ({})", stem, counter))
            } else {
                parent_dir.as_path().join(format!("{} ({}).{}", stem, counter, ext))
            };
            counter += 1;
        }

        if template.null_file {
            // Create empty file
            let file = File::create(&file_path).await?;
            file.sync_all().await?;
        } else if let Some(ref template_path) = template.template_path {
            // Copy from template
            tokio::fs::copy(template_path, &file_path).await?;
        }
        // If neither null_file nor template_path, create empty file anyway
        else {
            let file = File::create(&file_path).await?;
            file.sync_all().await?;
        }

        tracing::info!("Created new file from ShellNew template: {:?}", file_path);
        Ok(FilesPath::from(file_path.to_string_lossy().as_ref()))
    }

    /// Create from template (non-Windows stub)
    #[cfg(not(windows))]
    pub async fn create_from_template(&self, _parent_dir: &FilesPath, template: &ShellNewTemplate) -> Result<FilesPath> {
        Err(anyhow!("ShellNew is only available on Windows: {}", template.extension))
    }

    /// Get template by extension
    pub fn get_template_by_extension(&self, extension: &str) -> Option<ShellNewTemplate> {
        self.get_templates().ok()?.into_iter()
            .find(|t| t.extension.eq_ignore_ascii_case(extension))
    }

    /// Clear the templates cache (useful after system changes)
    pub fn clear_cache(&self) {
        #[cfg(windows)]
        {
            let mut cache = self.templates_cache.lock().unwrap();
            *cache = None;
        }
        tracing::debug!("ShellNew templates cache cleared");
    }
}

impl Default for ShellNewService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_shell_new_service_creation() {
        let service = ShellNewService::new();
        service.clear_cache();
    }

    #[test]
    fn test_get_templates_returns_vec() {
        let service = ShellNewService::new();
        let templates = service.get_templates();
        assert!(templates.is_ok());
        // On Windows this will have entries, on Linux empty
        let templates = templates.unwrap();
        tracing::debug!("Found {} ShellNew templates", templates.len());
    }

    #[test]
    fn test_get_template_by_extension() {
        let service = ShellNewService::new();
        // This test should work on both platforms
        let txt_template = service.get_template_by_extension(".txt");
        let _ = txt_template; // May be None on some systems
    }
}