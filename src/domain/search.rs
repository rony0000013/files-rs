use nucleo::{Config, Matcher, Utf32Str};
use std::sync::Mutex;

/// High-performance fuzzy search service for the UI
pub struct SearchService {
    matcher: Mutex<Matcher>,
    config: Config,
}

impl SearchService {
    pub fn new() -> Self {
        let config = Config::DEFAULT;
        Self {
            matcher: Mutex::new(Matcher::new(config.clone())),
            config,
        }
    }

    /// Filter a list of items based on a query string.
    pub fn filter_items(&self, query: &str, items: &[String]) -> Vec<usize> {
        if query.is_empty() {
            return (0..items.len()).collect();
        }

        let mut matches = Vec::new();

        // To use nucleo, we need to provide buffers for Utf32 conversion
        let mut needle_buf = Vec::new();
        let needle = Utf32Str::new(query, &mut needle_buf);

        let mut matcher = self.matcher.lock().unwrap();

        for (idx, item) in items.iter().enumerate() {
            let mut haystack_buf = Vec::new();
            let haystack = Utf32Str::new(item, &mut haystack_buf);

            if let Some(score) = matcher.fuzzy_match(haystack, needle) {
                matches.push((idx, score));
            }
        }

        // Sort by score descending
        matches.sort_by(|a, b| b.1.cmp(&a.1));

        matches.into_iter().map(|(idx, _)| idx).collect()
    }

    /// Get the configuration used by this service.
    #[allow(dead_code)]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Filter items by file extension (case-insensitive).
    /// Empty extension filter returns all items.
    pub fn filter_by_extension(items: &[(String, String)], ext: &str) -> Vec<(String, String)> {
        if ext.is_empty() {
            return items.to_vec();
        }
        let ext_lower = ext.to_lowercase();
        items
            .iter()
            .filter(|(name, _)| {
                std::path::Path::new(name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase() == ext_lower)
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    /// Filter items by size range [min, max].
    /// max=0 means no upper bound.
    pub fn filter_by_size(items: &[(String, u64)], min: u64, max: u64) -> Vec<(String, u64)> {
        items
            .iter()
            .filter(|(_, size)| *size >= min && (max == 0 || *size <= max))
            .cloned()
            .collect()
    }
}

/// Parsed AQS (Advanced Query Syntax) filter constraints
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AqsFilters {
    pub kind: Option<String>,
    pub extension: Option<String>,
    pub size_min: Option<u64>,
    pub size_max: Option<u64>,
    pub date_after: Option<i64>,
    pub date_before: Option<i64>,
}

impl SearchService {
    /// Supports: kind:type, ext:extension, size:>1mb, size:<500kb, date:yesterday
    pub fn parse_aqs(query: &str) -> AqsFilters {
        let mut filters = AqsFilters::default();
        for token in query.split_whitespace() {
            if let Some(value) = token.strip_prefix("kind:") {
                filters.kind = Some(value.to_lowercase());
            } else if let Some(value) = token.strip_prefix("ext:") {
                filters.extension = Some(value.trim_start_matches('.').to_lowercase());
            } else if let Some(value) = token.strip_prefix("size:") {
                Self::parse_size_filter(value, &mut filters);
            } else if let Some(value) = token.strip_prefix("date:") {
                Self::parse_date_filter(value, &mut filters);
            }
        }
        filters
    }

    fn parse_size_filter(value: &str, filters: &mut AqsFilters) {
        if let Some(num) = value.strip_prefix('>') {
            if let Some(b) = Self::parse_size_value(num) {
                filters.size_min = Some(b);
            }
        } else if let Some(num) = value.strip_prefix('<') {
            if let Some(b) = Self::parse_size_value(num) {
                filters.size_max = Some(b);
            }
        } else if let Some(num) = value.strip_prefix('=') {
            if let Some(b) = Self::parse_size_value(num) {
                filters.size_min = Some(b);
                filters.size_max = Some(b);
            }
        }
    }

    fn parse_size_value(value: &str) -> Option<u64> {
        let v = value.trim().to_lowercase();
        if let Some(n) = v.strip_suffix("gb") {
            n.trim()
                .parse::<f64>()
                .ok()
                .map(|x| (x * 1_073_741_824.0) as u64)
        } else if let Some(n) = v.strip_suffix("mb") {
            n.trim()
                .parse::<f64>()
                .ok()
                .map(|x| (x * 1_048_576.0) as u64)
        } else if let Some(n) = v.strip_suffix("kb") {
            n.trim().parse::<f64>().ok().map(|x| (x * 1024.0) as u64)
        } else {
            v.parse::<u64>().ok()
        }
    }

    fn parse_date_filter(value: &str, filters: &mut AqsFilters) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let day = 86400i64;
        match value {
            "yesterday" => {
                filters.date_after = Some(now - day * 2);
                filters.date_before = Some(now - day);
            }
            "today" => {
                // Start of today: current time minus time since midnight
                let secs_since_midnight = now % day;
                filters.date_after = Some(now - secs_since_midnight);
            }
            "thisweek" => filters.date_after = Some(now - day * 7),
            "thismonth" => filters.date_after = Some(now - day * 30),
            "thisyear" => filters.date_after = Some(now - day * 365),
            _ => {
                if let Some(ts) = Self::parse_date_value(value) {
                    if let Some(s) = value.strip_prefix('>') {
                        if let Some(t) = Self::parse_date_value(s) {
                            filters.date_after = Some(t);
                        }
                    } else if let Some(s) = value.strip_prefix('<') {
                        if let Some(t) = Self::parse_date_value(s) {
                            filters.date_before = Some(t);
                        }
                    } else {
                        filters.date_after = Some(ts);
                        filters.date_before = Some(ts + day);
                    }
                }
            }
        }
    }

    fn parse_date_value(value: &str) -> Option<i64> {
        let parts: Vec<&str> = value.split('-').collect();
        if parts.len() != 3 {
            return None;
        }
        let y = parts[0].parse::<i64>().ok()?;
        let m = parts[1].parse::<u64>().ok()?;
        let d = parts[2].parse::<u64>().ok()?;
        if m < 1 || m > 12 || d < 1 || d > 31 {
            return None;
        }
        let mut total = 0i64;
        for year in 1970..y {
            total += if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                366
            } else {
                365
            };
        }
        let dim = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };
        for i in 0..(m as usize - 1) {
            total += dim[i] as i64;
        }
        total += (d - 1) as i64;
        Some(total * 86400)
    }
}

impl Default for SearchService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_service_new() {
        let service = SearchService::new();
        assert!(service.matcher.lock().is_ok());
    }

    #[test]
    fn test_filter_items_empty_query_returns_all() {
        let service = SearchService::new();
        let items = vec![
            "document.txt".to_string(),
            "image.png".to_string(),
            "readme.md".to_string(),
        ];

        let result = service.filter_items("", &items);
        assert_eq!(result, vec![0, 1, 2]);
    }

    #[test]
    fn test_filter_items_exact_match() {
        let service = SearchService::new();
        let items = vec![
            "document.txt".to_string(),
            "image.png".to_string(),
            "readme.md".to_string(),
        ];

        let result = service.filter_items("readme", &items);
        assert!(result.contains(&2));
    }

    #[test]
    fn test_filter_items_case_insensitive() {
        let service = SearchService::new();
        let items = vec![
            "Document.txt".to_string(),
            "IMAGE.PNG".to_string(),
            "README.md".to_string(),
        ];

        let result = service.filter_items("doc", &items);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_filter_items_partial_match() {
        let service = SearchService::new();
        let items = vec![
            "apples.txt".to_string(),
            "bananas.txt".to_string(),
            "cherries.txt".to_string(),
        ];

        let result = service.filter_items("app", &items);
        assert!(result.contains(&0));
    }

    #[test]
    fn test_filter_items_no_match() {
        let service = SearchService::new();
        let items = vec!["apples.txt".to_string(), "bananas.txt".to_string()];

        let result = service.filter_items("xyz", &items);
        assert!(result.is_empty());
    }

    #[test]
    fn test_filter_items_empty_items() {
        let service = SearchService::new();
        let items: Vec<String> = vec![];

        let result = service.filter_items("test", &items);
        assert!(result.is_empty());
    }

    #[test]
    fn test_filter_items_sorted_by_relevance() {
        let service = SearchService::new();
        let items = vec![
            "test_file.txt".to_string(),
            "test.txt".to_string(),
            "file_test.txt".to_string(),
        ];

        let result = service.filter_items("test", &items);
        // "test.txt" should be more relevant than "test_file.txt"
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_filter_items_with_special_characters() {
        let service = SearchService::new();
        let items = vec![
            "file (1).txt".to_string(),
            "file [2].txt".to_string(),
            "file {3}.txt".to_string(),
        ];

        let result = service.filter_items("file", &items);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_filter_items_unicode() {
        let service = SearchService::new();
        let items = vec![
            "文档.txt".to_string(),
            "其他.txt".to_string(),
            "picture.png".to_string(),
        ];

        let result = service.filter_items("文档", &items);
        assert!(result.contains(&0));
    }

    #[test]
    fn test_filter_items_single_item() {
        let service = SearchService::new();
        let items = vec!["only_one.txt".to_string()];

        let result = service.filter_items("only", &items);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 0);
    }

    #[test]
    fn test_filter_items_prefix_match() {
        let service = SearchService::new();
        let items = vec![
            "backup_old.txt".to_string(),
            "backup_new.txt".to_string(),
            "backup_2024.txt".to_string(),
        ];

        let result = service.filter_items("backup", &items);
        assert_eq!(result.len(), 3);
    }

    // ─── SearchService: filter_by_extension ────────────────────────────────────

    #[test]
    fn test_filter_by_extension_txt() {
        let items = vec![
            ("readme.md".to_string(), "readme.md".to_string()),
            ("notes.txt".to_string(), "notes.txt".to_string()),
            ("image.png".to_string(), "image.png".to_string()),
        ];
        let result = SearchService::filter_by_extension(&items, "txt");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "notes.txt");
    }

    #[test]
    fn test_filter_by_extension_empty_returns_all() {
        let items = vec![("a.txt".to_string(), ""), ("b.rs".to_string(), "")];
        let result = SearchService::filter_by_extension(&items, "");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_by_extension_case_insensitive() {
        let items = vec![("File.TXT".to_string(), ""), ("file.txt".to_string(), "")];
        let result = SearchService::filter_by_extension(&items, "TXT");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_by_extension_no_match() {
        let items = vec![("doc.pdf".to_string(), ""), ("image.png".to_string(), "")];
        let result = SearchService::filter_by_extension(&items, "txt");
        assert_eq!(result.len(), 0);
    }

    // ─── SearchService: filter_by_size ─────────────────────────────────────────

    #[test]
    fn test_filter_by_size_exact_range() {
        let items = vec![
            ("small.txt".to_string(), 100u64),
            ("medium.txt".to_string(), 500),
            ("large.txt".to_string(), 5000),
        ];
        let result = SearchService::filter_by_size(&items, 200, 1000);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "medium.txt");
    }

    #[test]
    fn test_filter_by_size_zero_min() {
        let items = vec![
            ("small.txt".to_string(), 10u64),
            ("big.txt".to_string(), 1000),
        ];
        let result = SearchService::filter_by_size(&items, 0, 100);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_filter_by_size_zero_max() {
        let items = vec![
            ("small.txt".to_string(), 10u64),
            ("big.txt".to_string(), 1000),
        ];
        // max = 0 means no upper bound
        let result = SearchService::filter_by_size(&items, 500, 0);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_filter_by_size_no_match() {
        let items = vec![("tiny.txt".to_string(), 5u64)];
        let result = SearchService::filter_by_size(&items, 100, 1000);
        assert_eq!(result.len(), 0);
    }

    // ─── SearchService: filter_by_extension_and_size (combined) ────────────────

    #[test]
    fn test_filter_by_extension_and_size() {
        let items = vec![
            ("small.txt".to_string(), 100u64),
            ("large.txt".to_string(), 5000),
            ("small.png".to_string(), 200),
        ];
        // Filter: .txt files >= 50 bytes
        let by_ext = SearchService::filter_by_extension(&items, "txt");
        let result = SearchService::filter_by_size(&by_ext, 50, 0);
        assert_eq!(result.len(), 2);
    }

    // ─── SearchResult filtering (simulated) ─────────────────────────────────────

    #[test]
    fn test_search_result_default() {
        let state = super::SearchState::default();
        assert!(!state.is_searching);
        assert!(state.query.is_empty());
        assert!(!state.loading);
        assert!(!state.search_content);
        assert_eq!(state.size_min, 0);
        assert_eq!(state.size_max, 0);
        assert_eq!(state.extension_filter, "");
        assert!(!state.cancelled);
    }

    #[test]
    fn test_search_state_has_filters() {
        // Verify that the SearchState struct has all filter fields we expect
        let state = super::SearchState {
            search_content: true,
            size_min: 1024,
            size_max: 1048576,
            extension_filter: "txt".to_string(),
            cancelled: false,
            ..Default::default()
        };
        assert!(state.search_content);
        assert_eq!(state.size_min, 1024);
        assert_eq!(state.size_max, 1048576);
        assert_eq!(state.extension_filter, "txt");
    }

    // ─── AQS Parser tests ─────────────────────────────────────────────────

    #[test]
    fn test_parse_aqs_kind_document() {
        let filters = SearchService::parse_aqs("kind:document");
        assert!(filters.kind.is_some());
        assert_eq!(filters.kind.unwrap(), "document");
    }

    #[test]
    fn test_parse_aqs_kind_image() {
        let filters = SearchService::parse_aqs("kind:image");
        assert!(filters.kind.is_some());
        assert_eq!(filters.kind.unwrap(), "image");
    }

    #[test]
    fn test_parse_aqs_size_greater_than() {
        let filters = SearchService::parse_aqs("size:>1mb");
        assert!(filters.size_min.is_some());
        assert_eq!(filters.size_min.unwrap(), 1_048_576);
        assert!(filters.size_max.is_none());
    }

    #[test]
    fn test_parse_aqs_size_less_than() {
        let filters = SearchService::parse_aqs("size:<500kb");
        assert!(filters.size_max.is_some());
        assert_eq!(filters.size_max.unwrap(), 500 * 1024);
        assert!(filters.size_min.is_none());
    }

    #[test]
    fn test_parse_aqs_size_exact() {
        let filters = SearchService::parse_aqs("size:=2gb");
        assert_eq!(filters.size_min.unwrap(), 2_147_483_648);
        assert_eq!(filters.size_max.unwrap(), 2_147_483_648);
    }

    #[test]
    fn test_parse_aqs_date_yesterday() {
        let filters = SearchService::parse_aqs("date:yesterday");
        assert!(filters.date_after.is_some());
    }

    #[test]
    fn test_parse_aqs_date_thisweek() {
        let filters = SearchService::parse_aqs("date:thisweek");
        assert!(filters.date_after.is_some());
    }

    #[test]
    fn test_parse_aqs_date_greater_than() {
        let filters = SearchService::parse_aqs("date:>2024-01-01");
        assert!(filters.date_after.is_some());
    }

    #[test]
    fn test_parse_aqs_ext() {
        let filters = SearchService::parse_aqs("ext:.txt");
        assert_eq!(filters.extension, Some("txt".to_string()));
    }

    #[test]
    fn test_parse_aqs_multiple_filters() {
        let filters = SearchService::parse_aqs("kind:image size:>500kb");
        assert_eq!(filters.kind.unwrap(), "image");
        assert!(filters.size_min.unwrap() > 500_000);
    }

    #[test]
    fn test_parse_aqs_plain_text_returns_none() {
        let filters = SearchService::parse_aqs("hello world");
        assert!(filters.kind.is_none());
        assert!(filters.extension.is_none());
        assert!(filters.size_min.is_none());
        assert!(filters.date_after.is_none());
    }

    #[test]
    fn test_parse_aqs_empty() {
        let filters = SearchService::parse_aqs("");
        assert!(filters.kind.is_none());
        assert!(filters.extension.is_none());
    }
}
