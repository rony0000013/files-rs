use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use git2::{Repository, StatusOptions};
use std::fs;

#[derive(Debug, Clone, PartialEq)]
pub struct GitProperties {
    pub branch: String,
    pub status: String,
    pub last_commit: String,
}

/// Git status of a single file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitFileStatus {
    /// File is unmodified
    Clean,
    /// File has been modified
    Modified,
    /// File has been added to staging
    Added,
    /// File has been deleted
    Deleted,
    /// File is untracked
    Untracked,
    /// File has been renamed
    Renamed,
    /// File has been copied
    Copied,
    /// File has been ignored
    Ignored,
    /// Unknown status
    Unknown,
}

impl GitFileStatus {
    /// Get a short label/icon representation
    pub fn label(&self) -> &'static str {
        match self {
            GitFileStatus::Clean => "✓",
            GitFileStatus::Modified => "✎",
            GitFileStatus::Added => "+",
            GitFileStatus::Deleted => "✕",
            GitFileStatus::Untracked => "?",
            GitFileStatus::Renamed => "→",
            GitFileStatus::Copied => "⊕",
            GitFileStatus::Ignored => "!",
            GitFileStatus::Unknown => "·",
        }
    }

    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            GitFileStatus::Clean => "Clean",
            GitFileStatus::Modified => "Modified",
            GitFileStatus::Added => "Added",
            GitFileStatus::Deleted => "Deleted",
            GitFileStatus::Untracked => "Untracked",
            GitFileStatus::Renamed => "Renamed",
            GitFileStatus::Copied => "Copied",
            GitFileStatus::Ignored => "Ignored",
            GitFileStatus::Unknown => "Unknown",
        }
    }
}

/// Information about the last commit for a file
#[derive(Debug, Clone, PartialEq)]
pub struct GitCommitInfo {
    pub author: String,
    pub message: String,
    pub date: String,
}

pub struct GitService;

impl GitService {
    pub fn get_properties(path: &FilesPath) -> Result<Option<GitProperties>> {
        let repo = Repository::discover(path.as_path())?;
        let head = repo.head()?;
        let branch = head.shorthand().unwrap_or("unknown").to_string();

        let mut status_options = StatusOptions::new();
        status_options.include_untracked(true);
        let statuses = repo.statuses(Some(&mut status_options))?;
        let status_summary = if statuses.is_empty() {
            "Clean".to_string()
        } else {
            format!("{} changes", statuses.len())
        };

        let commit = repo.head()?.peel_to_commit()?;
        let last_commit = format!(
            "{} - {}",
            commit.id().to_string().get(0..7).unwrap_or(""),
            commit.summary().unwrap_or("No summary")
        );

        Ok(Some(GitProperties {
            branch,
            status: status_summary,
            last_commit,
        }))
    }

    /// Check if a path is inside a git repository
    pub fn is_repo(path: &FilesPath) -> bool {
        Repository::discover(path.as_path()).is_ok()
    }

    /// Get the branch name for the repo containing this path
    pub fn get_branch(path: &FilesPath) -> Option<String> {
        let repo = Repository::discover(path.as_path()).ok()?;
        let head = repo.head().ok()?;
        head.shorthand().map(|s| s.to_string())
    }

    /// Get the git status of a specific file within a repository
    pub fn get_file_status(path: &FilesPath) -> GitFileStatus {
        let repo = match Repository::discover(path.as_path()) {
            Ok(r) => r,
            Err(_) => return GitFileStatus::Unknown,
        };

        // Get the relative path within the repo
        let repo_path = match repo.workdir() {
            Some(p) => p,
            None => return GitFileStatus::Unknown,
        };

        let relative = match path.as_path().strip_prefix(repo_path) {
            Ok(r) => r,
            Err(_) => return GitFileStatus::Unknown,
        };

        match repo.status_file(relative) {
            Ok(status) => {
                use git2::Status;
                if status == Status::CURRENT {
                    GitFileStatus::Clean
                } else if status.contains(Status::WT_NEW) || status.contains(Status::INDEX_NEW) {
                    GitFileStatus::Added
                } else if status.contains(Status::WT_MODIFIED)
                    || status.contains(Status::INDEX_MODIFIED)
                {
                    GitFileStatus::Modified
                } else if status.contains(Status::WT_DELETED)
                    || status.contains(Status::INDEX_DELETED)
                {
                    GitFileStatus::Deleted
                } else if status.contains(Status::WT_RENAMED)
                    || status.contains(Status::INDEX_RENAMED)
                {
                    GitFileStatus::Renamed
                } else if status.contains(Status::WT_TYPECHANGE)
                    || status.contains(Status::INDEX_TYPECHANGE)
                {
                    GitFileStatus::Modified
                } else if status.contains(Status::IGNORED) {
                    GitFileStatus::Ignored
                } else if status.contains(Status::CONFLICTED) {
                    GitFileStatus::Modified
                } else {
                    GitFileStatus::Unknown
                }
            }
            Err(_) => GitFileStatus::Unknown,
        }
    }

    /// Get the last commit information for a specific file
    pub fn get_last_commit(path: &FilesPath) -> Option<GitCommitInfo> {
        let repo = Repository::discover(path.as_path()).ok()?;
        let head = repo.head().ok()?;
        let commit = head.peel_to_commit().ok()?;

        let author = commit.author().name().unwrap_or("Unknown").to_string();
        let message = commit.summary().unwrap_or("No message").to_string();

        // Format the commit time using standard library
        let time = commit.time();
        let seconds = time.seconds();
        let datetime = if seconds >= 0 {
            use std::time::{SystemTime, UNIX_EPOCH};
            if let Some(d) = UNIX_EPOCH.checked_add(std::time::Duration::from_secs(seconds as u64))
            {
                // Convert to time_t and format using localtime_r or simple string
                format_timestamp(d)
            } else {
                "Unknown date".to_string()
            }
        } else {
            "Unknown date".to_string()
        };

        Some(GitCommitInfo {
            author,
            message,
            date: datetime,
        })
    }
}

/// Format a SystemTime as YYYY-MM-DD HH:MM:SS using only std library
fn format_timestamp(t: std::time::SystemTime) -> String {
    use std::time::UNIX_EPOCH;
    let duration = t.duration_since(UNIX_EPOCH).unwrap_or_default();
    let total_secs = duration.as_secs();

    // Simple leap-year-aware date calculation
    let days = total_secs / 86400;
    let time_secs = total_secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Calculate year, month, day from days since epoch (1970-01-01)
    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }

    let month_days = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut m = 0usize;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining < md as i64 {
            m = i;
            break;
        }
        remaining -= md as i64;
    }

    let day = remaining + 1; // days are 1-indexed

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        y,
        m + 1,
        day,
        hours,
        minutes,
        seconds
    )
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_git_properties_creation() {
        let props = GitProperties {
            branch: "main".to_string(),
            status: "Clean".to_string(),
            last_commit: "abc1234 - Initial commit".to_string(),
        };

        assert_eq!(props.branch, "main");
        assert_eq!(props.status, "Clean");
        assert!(props.last_commit.contains("abc1234"));
    }

    #[test]
    fn test_git_properties_with_changes() {
        let props = GitProperties {
            branch: "feature/test".to_string(),
            status: "5 changes".to_string(),
            last_commit: "def5678 - Add new feature".to_string(),
        };

        assert_eq!(props.branch, "feature/test");
        assert_eq!(props.status, "5 changes");
        assert!(props.last_commit.contains("def5678"));
    }

    #[test]
    fn test_get_properties_nonexistent_path() {
        // Test that get_properties returns an error for a path that doesn't exist
        // or is not in a git repository
        let temp_dir = tempfile::TempDir::new().unwrap();
        let non_git_path = FilesPath::from(temp_dir.path().to_str().unwrap());

        // Should fail because the temp dir is not a git repository
        let result = GitService::get_properties(&non_git_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_git_properties_debug() {
        let props = GitProperties {
            branch: "develop".to_string(),
            status: "2 changes".to_string(),
            last_commit: "1234567 - Bug fix".to_string(),
        };

        let debug_str = format!("{:?}", props);
        assert!(debug_str.contains("develop"));
        assert!(debug_str.contains("2 changes"));
    }

    #[test]
    fn test_git_properties_clone() {
        let props1 = GitProperties {
            branch: "main".to_string(),
            status: "Clean".to_string(),
            last_commit: "abc123 - test".to_string(),
        };

        let props2 = props1.clone();
        assert_eq!(props1.branch, props2.branch);
        assert_eq!(props1.status, props2.status);
        assert_eq!(props1.last_commit, props2.last_commit);
    }

    #[test]
    fn test_git_properties_partial_eq() {
        let props1 = GitProperties {
            branch: "main".to_string(),
            status: "Clean".to_string(),
            last_commit: "abc123 - test".to_string(),
        };

        let props2 = GitProperties {
            branch: "main".to_string(),
            status: "Clean".to_string(),
            last_commit: "abc123 - test".to_string(),
        };

        let props3 = GitProperties {
            branch: "feature".to_string(),
            status: "Clean".to_string(),
            last_commit: "abc123 - test".to_string(),
        };

        assert_eq!(props1, props2);
        assert_ne!(props1, props3);
    }

    #[test]
    fn test_git_file_status_label() {
        // Verify each variant returns the correct icon
        assert_eq!(GitFileStatus::Clean.label(), "✓");
        assert_eq!(GitFileStatus::Modified.label(), "✎");
        assert_eq!(GitFileStatus::Added.label(), "+");
        assert_eq!(GitFileStatus::Deleted.label(), "✕");
        assert_eq!(GitFileStatus::Untracked.label(), "?");
        assert_eq!(GitFileStatus::Renamed.label(), "→");
        assert_eq!(GitFileStatus::Copied.label(), "⊕");
        assert_eq!(GitFileStatus::Ignored.label(), "!");
        assert_eq!(GitFileStatus::Unknown.label(), "·");
    }

    #[test]
    fn test_git_service_get_branch_on_real_repo() {
        // Create a temp directory and init a git repo
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize a git repository
        let repo = git2::Repository::init(repo_path).unwrap();

        // Create an initial commit so we have a HEAD with a branch
        let sig = git2::Signature::now("Test", "test@test.com").unwrap();
        let tree_id = {
            let index = repo.index().unwrap();
            let oid = index.write_tree().unwrap();
            repo.find_tree(oid).unwrap().id()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        // Now test get_branch
        let files_path = FilesPath::from(repo_path.to_str().unwrap());
        let branch = GitService::get_branch(&files_path);
        assert!(branch.is_some());
        assert_eq!(branch.unwrap(), "master"); // default branch name
    }

    #[test]
    fn test_get_last_commit_for_file() {
        // Create a temp directory and init a git repo
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize a git repository
        let repo = git2::Repository::init(repo_path).unwrap();

        // Create a file and add it to the index
        let file_path = repo_path.join("test.txt");
        std::fs::write(&file_path, b"Hello, world!").unwrap();

        // Stage the file
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("test.txt")).unwrap();
        index.write().unwrap();

        // Get the tree
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        // Commit
        let sig = git2::Signature::now("Test Author", "test@test.com").unwrap();
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "My test commit message",
            &tree,
            &[],
        )
        .unwrap();

        // Now test get_last_commit for the file
        let files_path = FilesPath::from(file_path.to_str().unwrap());
        let commit_info = GitService::get_last_commit(&files_path);

        assert!(commit_info.is_some());
        let info = commit_info.unwrap();
        assert_eq!(info.author, "Test Author");
        assert!(info.message.contains("My test commit message"));
        assert!(!info.date.is_empty());
    }
}
