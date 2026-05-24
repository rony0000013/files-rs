//! Adaptive View Service
//! Dynamically selects the most appropriate layout mode based on folder content type.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// View mode types for the adaptive system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdaptiveViewMode {
    Details,
    Grid,
    Columns,
    Cards,
    Compact,
}

impl Default for AdaptiveViewMode {
    fn default() -> Self {
        Self::Details
    }
}

/// Content analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentAnalysis {
    /// Number of items in the folder
    pub item_count: usize,
    /// Number of subdirectories
    pub folder_count: usize,
    /// Number of image files
    pub image_count: usize,
    /// Number of video files
    pub video_count: usize,
    /// Number of audio files
    pub audio_count: usize,
    /// Number of document files
    pub document_count: usize,
    /// Number of code files
    pub code_count: usize,
    /// Number of archive files
    pub archive_count: usize,
    /// Average file size in bytes
    pub avg_file_size: u64,
    /// Has hidden items
    pub has_hidden_items: bool,
    /// Is a git repository
    pub is_git_repo: bool,
}

impl Default for ContentAnalysis {
    fn default() -> Self {
        Self {
            item_count: 0,
            folder_count: 0,
            image_count: 0,
            video_count: 0,
            audio_count: 0,
            document_count: 0,
            code_count: 0,
            archive_count: 0,
            avg_file_size: 0,
            has_hidden_items: false,
            is_git_repo: false,
        }
    }
}

impl ContentAnalysis {
    /// Create an analysis from a list of file extensions
    pub fn from_extensions(extensions: &[String], item_count: usize, avg_file_size: u64) -> Self {
        let mut analysis = Self::default();
        analysis.item_count = item_count;
        analysis.avg_file_size = avg_file_size;

        for ext in extensions {
            let ext_lower = ext.to_lowercase();
            match ext_lower.as_str() {
                // Images
                "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tiff"
                | "tif" | "webp" => {
                    analysis.image_count += 1;
                }
                // Videos
                "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpeg" | "mpg" => {
                    analysis.video_count += 1;
                }
                // Audio
                "mp3" | "wav" | "flac" | "aac" | "ogg" | "wma" | "m4a" | "opus" => {
                    analysis.audio_count += 1;
                }
                // Documents
                "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "rtf"
                | "odt" => {
                    analysis.document_count += 1;
                }
                // Code
                "rs" | "js" | "ts" | "py" | "java" | "c" | "cpp" | "h" | "cs" | "go" | "rb"
                | "php" | "html" | "css" | "json" | "xml" | "yaml" | "yml" | "toml" | "md" => {
                    analysis.code_count += 1;
                }
                // Archives
                "zip" | "7z" | "rar" | "tar" | "gz" | "bz2" | "xz" => {
                    analysis.archive_count += 1;
                }
                _ => {}
            }
        }

        analysis
    }

    /// Get the dominant content type as a percentage
    pub fn dominant_type_percentage(&self) -> f32 {
        if self.item_count == 0 {
            return 0.0;
        }

        let max_count = [
            self.image_count,
            self.video_count,
            self.audio_count,
            self.document_count,
            self.code_count,
        ]
        .into_iter()
        .max()
        .unwrap_or(0);

        (max_count as f32 / self.item_count as f32) * 100.0
    }

    /// Check if the content is primarily images (>60%)
    pub fn is_image_heavy(&self) -> bool {
        self.item_count > 0 && self.image_count > self.item_count / 2
    }

    /// Check if the content is primarily code (>60%)
    pub fn is_code_heavy(&self) -> bool {
        self.item_count > 0 && self.code_count > self.item_count / 2
    }

    /// Check if the content is primarily documents (>60%)
    pub fn is_document_heavy(&self) -> bool {
        self.item_count > 0 && self.document_count > self.item_count / 2
    }

    /// Check if the content is primarily media (images, videos, audio)
    pub fn is_media_heavy(&self) -> bool {
        let media_count = self.image_count + self.video_count + self.audio_count;
        self.item_count > 0 && media_count > self.item_count / 2
    }

    /// Check if should use compact view (many items, small files)
    pub fn should_use_compact(&self) -> bool {
        self.item_count > 500 && self.avg_file_size < 1024 * 1024 // 1MB
    }
}

/// Adaptive View Service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveViewService {
    /// User's manual preference (None = automatic)
    pub manual_override: Option<AdaptiveViewMode>,
    /// History of view mode selections per folder pattern
    pub view_history: HashMap<String, AdaptiveViewMode>,
    /// Enable adaptive switching
    pub enabled: bool,
    /// Confidence threshold for automatic switching (0-100)
    pub confidence_threshold: u8,
}

impl Default for AdaptiveViewService {
    fn default() -> Self {
        Self {
            manual_override: None,
            view_history: HashMap::new(),
            enabled: true,
            confidence_threshold: 70,
        }
    }
}

impl AdaptiveViewService {
    /// Analyze content and suggest the best view mode
    pub fn analyze_and_suggest(&self, analysis: &ContentAnalysis) -> (AdaptiveViewMode, u8) {
        // If manual override is set, use it with high confidence
        if let Some(mode) = self.manual_override {
            return (mode, 100);
        }

        // Small folder - use details view
        if analysis.item_count <= 10 {
            return (AdaptiveViewMode::Details, 90);
        }

        // Many items, small files - compact view
        if analysis.should_use_compact() {
            return (AdaptiveViewMode::Compact, 75);
        }

        // Image-heavy folder - grid view for visual browsing
        if analysis.is_image_heavy() {
            return (AdaptiveViewMode::Grid, 85);
        }

        // Code-heavy folder - details view for sorting and navigation
        if analysis.is_code_heavy() {
            return (AdaptiveViewMode::Details, 80);
        }

        // Document-heavy folder - details view
        if analysis.is_document_heavy() {
            return (AdaptiveViewMode::Details, 75);
        }

        // Media-heavy folder - grid view
        if analysis.is_media_heavy() {
            return (AdaptiveViewMode::Grid, 70);
        }

        // Codebase/Git repos - columns view for navigation
        if analysis.is_git_repo {
            return (AdaptiveViewMode::Columns, 80);
        }

        // Default to details view
        (AdaptiveViewMode::Details, 60)
    }

    /// Suggest view mode based on folder path pattern
    pub fn suggest_for_path(&self, path: &str) -> Option<AdaptiveViewMode> {
        // Check history for similar paths
        for (pattern, mode) in &self.view_history {
            if path.contains(pattern) {
                return Some(*mode);
            }
        }
        None
    }

    /// Record user's view mode selection for a folder
    pub fn record_selection(&mut self, path: &str, mode: AdaptiveViewMode) {
        // Extract a meaningful pattern from the path
        // e.g., "C:\Users\Documents\Projects\MyProject\src" -> "Projects\MyProject"
        let parts: Vec<&str> = path.split(['\\', '/']).collect();

        // Use the last 2-3 meaningful segments as pattern
        if parts.len() >= 2 {
            let pattern = parts[parts.len() - 2..].join("\\");
            self.view_history.insert(pattern, mode);
        }
    }

    /// Set manual view mode override
    pub fn set_override(&mut self, mode: Option<AdaptiveViewMode>) {
        self.manual_override = mode;
    }

    /// Clear all history
    pub fn clear_history(&mut self) {
        self.view_history.clear();
    }

    /// Toggle adaptive mode
    pub fn toggle_enabled(&mut self) {
        self.enabled = !self.enabled;
    }

    /// Set confidence threshold
    pub fn set_confidence_threshold(&mut self, threshold: u8) {
        self.confidence_threshold = threshold.min(100);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_view_mode_default() {
        assert_eq!(AdaptiveViewMode::default(), AdaptiveViewMode::Details);
    }

    #[test]
    fn test_content_analysis_default() {
        let analysis = ContentAnalysis::default();
        assert_eq!(analysis.item_count, 0);
        assert_eq!(analysis.folder_count, 0);
        assert_eq!(analysis.image_count, 0);
        assert!(!analysis.has_hidden_items);
        assert!(!analysis.is_git_repo);
    }

    #[test]
    fn test_content_analysis_from_extensions() {
        let extensions = vec![
            "rs".to_string(),
            "rs".to_string(),
            "js".to_string(),
            "md".to_string(),
        ];
        let analysis = ContentAnalysis::from_extensions(&extensions, 4, 1000);
        assert_eq!(analysis.item_count, 4);
        assert_eq!(analysis.code_count, 4); // rs, rs, js, md all count as code
    }

    #[test]
    fn test_content_analysis_image_heavy() {
        let mut analysis = ContentAnalysis::default();
        analysis.item_count = 10;
        analysis.image_count = 6;
        assert!(analysis.is_image_heavy());

        analysis.image_count = 5;
        assert!(!analysis.is_image_heavy()); // 50% is not > 50%
    }

    #[test]
    fn test_content_analysis_code_heavy() {
        let mut analysis = ContentAnalysis::default();
        analysis.item_count = 10;
        analysis.code_count = 7;
        assert!(analysis.is_code_heavy());
    }

    #[test]
    fn test_content_analysis_document_heavy() {
        let mut analysis = ContentAnalysis::default();
        analysis.item_count = 10;
        analysis.document_count = 6;
        assert!(analysis.is_document_heavy());
    }

    #[test]
    fn test_content_analysis_media_heavy() {
        let mut analysis = ContentAnalysis::default();
        analysis.item_count = 10;
        analysis.image_count = 3;
        analysis.video_count = 3;
        assert!(analysis.is_media_heavy()); // 6/10 > 5/10
    }

    #[test]
    fn test_content_analysis_should_use_compact() {
        let mut analysis = ContentAnalysis::default();
        analysis.item_count = 600;
        analysis.avg_file_size = 500_000; // 500KB < 1MB
        assert!(analysis.should_use_compact());

        analysis.avg_file_size = 2 * 1024 * 1024; // 2MB
        assert!(!analysis.should_use_compact());
    }

    #[test]
    fn test_content_analysis_dominant_type_percentage() {
        let mut analysis = ContentAnalysis::default();
        analysis.item_count = 10;
        analysis.image_count = 7;
        assert_eq!(analysis.dominant_type_percentage(), 70.0);

        analysis.item_count = 0;
        assert_eq!(analysis.dominant_type_percentage(), 0.0);
    }

    #[test]
    fn test_adaptive_view_service_default() {
        let service = AdaptiveViewService::default();
        assert!(service.manual_override.is_none());
        assert!(service.view_history.is_empty());
        assert!(service.enabled);
        assert_eq!(service.confidence_threshold, 70);
    }

    #[test]
    fn test_adaptive_view_service_analyze_and_suggest_small_folder() {
        let service = AdaptiveViewService::default();
        let mut analysis = ContentAnalysis::default();
        analysis.item_count = 5;

        let (mode, confidence) = service.analyze_and_suggest(&analysis);
        assert_eq!(mode, AdaptiveViewMode::Details);
        assert_eq!(confidence, 90);
    }

    #[test]
    fn test_adaptive_view_service_analyze_and_suggest_image_heavy() {
        let service = AdaptiveViewService::default();
        let mut analysis = ContentAnalysis::default();
        analysis.item_count = 20;
        analysis.image_count = 15;

        let (mode, confidence) = service.analyze_and_suggest(&analysis);
        assert_eq!(mode, AdaptiveViewMode::Grid);
        assert_eq!(confidence, 85);
    }

    #[test]
    fn test_adaptive_view_service_analyze_and_suggest_code_heavy() {
        let service = AdaptiveViewService::default();
        let mut analysis = ContentAnalysis::default();
        analysis.item_count = 100;
        analysis.code_count = 70;

        let (mode, confidence) = service.analyze_and_suggest(&analysis);
        assert_eq!(mode, AdaptiveViewMode::Details);
        assert_eq!(confidence, 80);
    }

    #[test]
    fn test_adaptive_view_service_analyze_and_suggest_git_repo() {
        let service = AdaptiveViewService::default();
        let mut analysis = ContentAnalysis::default();
        analysis.item_count = 50;
        analysis.is_git_repo = true;

        let (mode, confidence) = service.analyze_and_suggest(&analysis);
        assert_eq!(mode, AdaptiveViewMode::Columns);
        assert_eq!(confidence, 80);
    }

    #[test]
    fn test_adaptive_view_service_analyze_and_suggest_compact() {
        let service = AdaptiveViewService::default();
        let mut analysis = ContentAnalysis::default();
        analysis.item_count = 600;
        analysis.avg_file_size = 500; // Small files

        let (mode, confidence) = service.analyze_and_suggest(&analysis);
        assert_eq!(mode, AdaptiveViewMode::Compact);
        assert_eq!(confidence, 75);
    }

    #[test]
    fn test_adaptive_view_service_analyze_with_manual_override() {
        let mut service = AdaptiveViewService::default();
        service.manual_override = Some(AdaptiveViewMode::Grid);

        let analysis = ContentAnalysis::default();
        let (mode, confidence) = service.analyze_and_suggest(&analysis);
        assert_eq!(mode, AdaptiveViewMode::Grid);
        assert_eq!(confidence, 100); // Full confidence for manual
    }

    #[test]
    fn test_adaptive_view_service_record_selection() {
        let mut service = AdaptiveViewService::default();
        service.record_selection(
            "C:\\Users\\Test\\Documents\\Project",
            AdaptiveViewMode::Grid,
        );
        assert!(service.view_history.len() <= 1);
    }

    #[test]
    fn test_adaptive_view_service_set_override() {
        let mut service = AdaptiveViewService::default();
        assert!(service.manual_override.is_none());

        service.set_override(Some(AdaptiveViewMode::Cards));
        assert_eq!(service.manual_override, Some(AdaptiveViewMode::Cards));

        service.set_override(None);
        assert!(service.manual_override.is_none());
    }

    #[test]
    fn test_adaptive_view_service_clear_history() {
        let mut service = AdaptiveViewService::default();
        service.record_selection("C:\\Test\\Path", AdaptiveViewMode::Grid);
        assert!(!service.view_history.is_empty());

        service.clear_history();
        assert!(service.view_history.is_empty());
    }

    #[test]
    fn test_adaptive_view_service_toggle_enabled() {
        let mut service = AdaptiveViewService::default();
        assert!(service.enabled);
        service.toggle_enabled();
        assert!(!service.enabled);
    }

    #[test]
    fn test_adaptive_view_service_set_confidence_threshold() {
        let mut service = AdaptiveViewService::default();
        assert_eq!(service.confidence_threshold, 70);

        service.set_confidence_threshold(90);
        assert_eq!(service.confidence_threshold, 90);

        // Clamp to 100
        service.set_confidence_threshold(150);
        assert_eq!(service.confidence_threshold, 100);
    }
}
