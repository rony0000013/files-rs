//! Directory enumeration benchmarks.
//! Provides helper functions to measure directory listing performance
//! and compare against baseline expectations.

use std::path::Path;
use std::time::{Duration, Instant};

/// Result of a directory listing benchmark.
#[derive(Debug, Clone)]
pub struct BenchResult {
    pub method: &'static str,
    pub path: String,
    pub file_count: usize,
    pub duration: Duration,
    pub files_per_second: f64,
}

impl BenchResult {
    pub fn new(method: &'static str, path: &Path, file_count: usize, duration: Duration) -> Self {
        let secs = duration.as_secs_f64();
        let fps = if secs > 0.0 {
            file_count as f64 / secs
        } else {
            0.0
        };
        Self {
            method,
            path: path.to_string_lossy().into_owned(),
            file_count,
            duration,
            files_per_second: fps,
        }
    }
}

/// Time how long it takes to enumerate a directory using jwalk.
pub fn bench_jwalk(path: &Path) -> Result<BenchResult, String> {
    let start = Instant::now();
    let mut count = 0;
    for entry in jwalk::WalkDir::new(path).max_depth(1).into_iter() {
        if entry.is_ok() {
            count += 1;
        }
    }
    let duration = start.elapsed();
    Ok(BenchResult::new("jwalk", path, count, duration))
}

/// Time how long it takes to enumerate a directory using std::fs::read_dir.
pub fn bench_std_fs(path: &Path) -> Result<BenchResult, String> {
    let start = Instant::now();
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries {
            if entry.is_ok() {
                count += 1;
            }
        }
    }
    let duration = start.elapsed();
    Ok(BenchResult::new("std::fs", path, count, duration))
}

/// Run both benchmarks and return comparison.
pub fn compare_methods(path: &Path) -> Result<(BenchResult, BenchResult), String> {
    let jwalk_result = bench_jwalk(path)?;
    let std_result = bench_std_fs(path)?;
    Ok((jwalk_result, std_result))
}

/// Print a formatted benchmark comparison.
pub fn print_comparison(jwalk: &BenchResult, std_fs: &BenchResult) {
    println!("Directory enumeration comparison:");
    println!("  Path: {}", jwalk.path);
    println!("  Files: {}", jwalk.file_count);
    println!("  Method   | Duration    | Files/sec");
    println!("  ---------+-------------+----------");
    println!(
        "  jwalk    | {:>10.3}s | {:>8.1}",
        jwalk.duration.as_secs_f64(),
        jwalk.files_per_second
    );
    println!(
        "  std::fs  | {:>10.3}s | {:>8.1}",
        std_fs.duration.as_secs_f64(),
        std_fs.files_per_second
    );
    if std_fs.duration.as_secs_f64() > 0.0 {
        let ratio = jwalk.duration.as_secs_f64() / std_fs.duration.as_secs_f64();
        println!(
            "  Ratio: jwalk is {:.2}x {}",
            ratio,
            if ratio > 1.0 { "slower" } else { "faster" }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_bench_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let result = bench_jwalk(tmp.path()).unwrap();
        assert_eq!(result.method, "jwalk");
        assert!(result.file_count >= 0);
        assert!(result.duration.as_secs_f64() >= 0.0);
    }

    #[test]
    fn test_bench_std_fs_empty() {
        let tmp = TempDir::new().unwrap();
        let result = bench_std_fs(tmp.path()).unwrap();
        assert_eq!(result.method, "std::fs");
    }

    #[test]
    fn test_bench_non_existent_path() {
        let result = bench_jwalk(Path::new("/nonexistent_path_xyz"));
        assert!(result.is_err());
    }

    #[test]
    fn test_compare_methods() {
        let tmp = TempDir::new().unwrap();
        for i in 0..5 {
            std::fs::write(tmp.path().join(format!("f{}.txt", i)), "data").unwrap();
        }
        let (jwalk_res, std_res) = compare_methods(tmp.path()).unwrap();
        assert_eq!(jwalk_res.file_count, 5);
        assert_eq!(std_res.file_count, 5);
    }

    #[test]
    fn test_bench_result_formatting() {
        let result = BenchResult::new("test", Path::new("/tmp"), 100, Duration::from_millis(500));
        assert!(result.files_per_second > 0.0);
        assert_eq!(result.file_count, 100);
        assert_eq!(result.method, "test");
    }

    #[test]
    fn test_print_comparison_no_panic() {
        let j = BenchResult::new("jwalk", Path::new("/tmp"), 100, Duration::from_millis(200));
        let s = BenchResult::new(
            "std::fs",
            Path::new("/tmp"),
            100,
            Duration::from_millis(300),
        );
        // Just verify no panic
        print_comparison(&j, &s);
    }

    #[test]
    fn test_bench_result_zero_duration() {
        let result = BenchResult::new("zero", Path::new("/"), 0, Duration::ZERO);
        assert_eq!(result.files_per_second, 0.0);
    }
}
