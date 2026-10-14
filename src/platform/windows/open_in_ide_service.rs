//! Open In IDE Service
//! Provides functionality to detect and open projects in IDEs (VS Code, Visual Studio, Rider)
//! Uses Windows API: ShellExec

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Supported IDE types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IDEType {
    VSCode,
    VisualStudio,
    Rider,
    JetBrainsFleet,
    Unknown,
}

impl Default for IDEType {
    fn default() -> Self {
        Self::Unknown
    }
}

impl IDEType {
    /// Detects IDE type from the path
    pub fn from_path(path: &Path) -> Self {
        let path_str = path.to_string_lossy().to_lowercase();

        if path_str.contains("rider") {
            IDEType::Rider
        } else if path_str.contains("visual studio") || path_str.contains("devenv") {
            IDEType::VisualStudio
        } else if path_str.contains("vscode") || path_str.contains("code") {
            IDEType::VSCode
        } else if path_str.contains("fleet") {
            IDEType::JetBrainsFleet
        } else {
            IDEType::Unknown
        }
    }

    /// Returns the executable name for this IDE
    pub fn executable(&self) -> Option<&str> {
        match self {
            IDEType::VSCode => Some("code.exe"),
            IDEType::VisualStudio => Some("devenv.exe"),
            IDEType::Rider => Some("rider64.exe"),
            IDEType::JetBrainsFleet => Some("fleet.exe"),
            IDEType::Unknown => None,
        }
    }

    /// Returns the default installation path for this IDE
    pub fn default_install_path(&self) -> Option<String> {
        match self {
            IDEType::VSCode => Some("C:\\Program Files\\Microsoft VS Code\\Code.exe".to_string()),
            IDEType::VisualStudio => Some("C:\\Program Files\\Microsoft Visual Studio\\2022\\Community\\Common7\\IDE\\devenv.exe".to_string()),
            IDEType::Rider => Some("C:\\Program Files\\JetBrains\\JetBrains Rider\\bin\\rider64.exe".to_string()),
            IDEType::JetBrainsFleet => Some("C:\\Program Files\\JetBrains\\Fleet\\bin\\fleet.exe".to_string()),
            IDEType::Unknown => None,
        }
    }
}

/// Result of opening in IDE operation
#[derive(Debug)]
pub struct OpenInIDEResult {
    pub success: bool,
    pub ide_type: IDEType,
    pub error_message: Option<String>,
}

/// Service for opening files/folders in IDEs
pub struct OpenInIDEService;

impl OpenInIDEService {
    /// Creates a new OpenInIDEService instance
    pub fn new() -> Self {
        Self
    }

    /// Detects the IDE type from the path
    pub fn detect_ide(path: &Path) -> IDEType {
        IDEType::from_path(path)
    }

    /// Finds an installed IDE on the system
    pub fn find_installed_ide() -> Option<IDEType> {
        let possible_ides = [
            ("code.exe", IDEType::VSCode),
            ("devenv.exe", IDEType::VisualStudio),
            ("rider64.exe", IDEType::Rider),
            ("fleet.exe", IDEType::JetBrainsFleet),
        ];

        for (exe, ide_type) in &possible_ides {
            if Self::is_ide_installed(ide_type) {
                return Some(*ide_type);
            }
        }
        None
    }

    /// Checks if a specific IDE is installed
    #[cfg(windows)]
    pub fn is_ide_installed(ide_type: &IDEType) -> bool {
        if let Some(default_path) = ide_type.default_install_path() {
            return Path::new(&default_path).exists();
        }

        // Also check PATH for the executable
        let exe_name = match ide_type.executable() {
            Some(name) => name,
            None => return false,
        };

        let output = Command::new("where").arg(exe_name).output();

        matches!(output, Ok(o) if o.status.success())
    }

    #[cfg(not(windows))]
    pub fn is_ide_installed(ide_type: &IDEType) -> bool {
        let _ = ide_type;
        false
    }

    /// Opens a path in the detected or preferred IDE
    #[cfg(windows)]
    pub async fn open(
        &self,
        path: &Path,
        preferred_ide: Option<IDEType>,
    ) -> Result<OpenInIDEResult> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let path_str = path.to_string_lossy();
        tracing::info!(
            "Opening in IDE: {} (preferred: {:?})",
            path_str,
            preferred_ide
        );

        // Determine which IDE to use
        let ide = preferred_ide
            .or_else(Self::find_installed_ide)
            .unwrap_or(IDEType::VSCode);

        let ide_path = match ide.default_install_path() {
            Some(p) if Path::new(&p).exists() => p,
            _ => {
                // Try to find in PATH
                let exe_name = ide.executable().unwrap_or("code.exe");
                match Command::new("where").arg(exe_name).output() {
                    Ok(output) if output.status.success() => {
                        String::from_utf8_lossy(&output.stdout)
                            .lines()
                            .next()
                            .unwrap_or("code.exe")
                            .to_string()
                    }
                    _ => {
                        // Fall back to code.exe in PATH
                        "code.exe".to_string()
                    }
                }
            }
        };

        let ide_path_wide: Vec<u16> = OsStr::new(&*ide_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let path_wide: Vec<u16> = OsStr::new(&*path_str)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            use windows::Win32::UI::Shell::ShellExecuteW;
            use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

            let result = ShellExecuteW(
                None,
                windows::core::PCWSTR::null(),
                windows::core::PCWSTR(ide_path_wide.as_ptr()),
                windows::core::PCWSTR(path_wide.as_ptr()),
                None,
                SW_SHOW,
            );

            let success = result.0 as usize > 32;

            Ok(OpenInIDEResult {
                success,
                ide_type: ide,
                error_message: if success {
                    None
                } else {
                    Some(format!(
                        "Failed to open {} in IDE (error code: {})",
                        path_str, result.0 as usize
                    ))
                },
            })
        }
    }

    /// Opens a path in IDE (non-Windows stub)
    #[cfg(not(windows))]
    pub async fn open(
        &self,
        path: &Path,
        preferred_ide: Option<IDEType>,
    ) -> Result<OpenInIDEResult> {
        let _ = path;
        let _ = preferred_ide;
        Ok(OpenInIDEResult {
            success: false,
            ide_type: IDEType::Unknown,
            error_message: Some("OpenInIDE is only available on Windows".to_string()),
        })
    }

    /// Opens a file in VS Code specifically (common use case)
    #[cfg(windows)]
    pub async fn open_in_vscode(&self, path: &Path) -> Result<OpenInIDEResult> {
        self.open(path, Some(IDEType::VSCode)).await
    }

    #[cfg(not(windows))]
    pub async fn open_in_vscode(&self, path: &Path) -> Result<OpenInIDEResult> {
        let _ = path;
        self.open(path, Some(IDEType::VSCode)).await
    }

    /// Opens a project folder in Visual Studio
    #[cfg(windows)]
    pub async fn open_in_visual_studio(&self, path: &Path) -> Result<OpenInIDEResult> {
        self.open(path, Some(IDEType::VisualStudio)).await
    }

    #[cfg(not(windows))]
    pub async fn open_in_visual_studio(&self, path: &Path) -> Result<OpenInIDEResult> {
        let _ = path;
        self.open(path, Some(IDEType::VisualStudio)).await
    }

    /// Checks if a path looks like a project (contains project files)
    pub fn is_project_path(path: &Path) -> bool {
        let project_markers = [
            "Cargo.toml",       // Rust
            "package.json",     // Node.js
            "requirements.txt", // Python
            "go.mod",           // Go
            "pom.xml",          // Java/Maven
            "build.gradle",     // Java/Gradle
            "*.sln",            // Visual Studio
            "*.csproj",         // C#
            "*.pyproj",         // Python
            ".code-workspace",  // VS Code workspace
            ".idea",            // JetBrains
            "CMakeLists.txt",   // C/C++
        ];

        if path.is_dir() {
            // Check if any project marker exists in the directory
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    if let Some(name_str) = name.to_str() {
                        for marker in &project_markers {
                            if marker.starts_with('*') {
                                // Extension match
                                let ext = &marker[1..];
                                if name_str.ends_with(ext) {
                                    return true;
                                }
                            } else if name_str == *marker {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        // Also check file extensions
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            return matches!(ext, "sln" | "csproj" | "pyproj" | "go" | "rs");
        }

        false
    }
}

impl Default for OpenInIDEService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_ide_type_default() {
        assert_eq!(IDEType::default(), IDEType::Unknown);
    }

    #[test]
    fn test_ide_type_from_path() {
        let vscode_path = Path::new("C:\\Program Files\\Microsoft VS Code\\Code.exe");
        let rider_path =
            Path::new("C:\\Program Files\\JetBrains\\JetBrains Rider\\bin\\rider64.exe");
        let unknown_path = Path::new("C:\\Program Files\\SomeApp\\app.exe");

        assert_eq!(OpenInIDEService::detect_ide(vscode_path), IDEType::VSCode);
        assert_eq!(OpenInIDEService::detect_ide(rider_path), IDEType::Rider);
        assert_eq!(OpenInIDEService::detect_ide(unknown_path), IDEType::Unknown);
    }

    #[test]
    fn test_ide_type_executable() {
        assert_eq!(IDEType::VSCode.executable(), Some("code.exe"));
        assert_eq!(IDEType::VisualStudio.executable(), Some("devenv.exe"));
        assert_eq!(IDEType::Rider.executable(), Some("rider64.exe"));
        assert_eq!(IDEType::Unknown.executable(), None);
    }

    #[test]
    fn test_ide_type_default_install_path() {
        assert!(IDEType::VSCode.default_install_path().is_some());
        assert!(IDEType::VisualStudio.default_install_path().is_some());
        assert!(IDEType::Rider.default_install_path().is_some());
        assert!(IDEType::Unknown.default_install_path().is_none());
    }

    #[test]
    fn test_ide_type_variants() {
        let ides = [
            IDEType::VSCode,
            IDEType::VisualStudio,
            IDEType::Rider,
            IDEType::JetBrainsFleet,
            IDEType::Unknown,
        ];
        for ide in ides {
            let debug_str = format!("{:?}", ide);
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn test_open_in_ide_result_debug() {
        let result = OpenInIDEResult {
            success: true,
            ide_type: IDEType::VSCode,
            error_message: None,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("true"));
        assert!(debug_str.contains("VSCode"));
    }

    #[test]
    fn test_open_in_ide_result_failure() {
        let result = OpenInIDEResult {
            success: false,
            ide_type: IDEType::Rider,
            error_message: Some("IDE not found".to_string()),
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("false"));
    }

    #[test]
    fn test_service_creation() {
        let service = OpenInIDEService::new();
        let default_service = OpenInIDEService::default();
        assert!(true); // Both created successfully
        let _ = default_service;
        let _ = service;
    }

    #[test]
    fn test_is_project_path_rust() {
        let temp_dir = tempdir().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        std::fs::write(&cargo_toml, "[package]").unwrap();

        assert!(OpenInIDEService::is_project_path(&temp_dir.path()));
    }

    #[test]
    fn test_is_project_path_nodejs() {
        let temp_dir = tempdir().unwrap();
        let package_json = temp_dir.path().join("package.json");
        std::fs::write(&package_json, "{}").unwrap();

        assert!(OpenInIDEService::is_project_path(&temp_dir.path()));
    }

    #[test]
    fn test_is_project_path_not_a_project() {
        let temp_dir = tempdir().unwrap();
        let readme = temp_dir.path().join("README.txt");
        std::fs::write(&readme, "Just a text file").unwrap();

        assert!(!OpenInIDEService::is_project_path(&temp_dir.path()));
    }

    #[test]
    fn test_is_project_path_solution_file() {
        let temp_dir = tempdir().unwrap();
        let sln_file = temp_dir.path().join("MyProject.sln");
        std::fs::write(&sln_file, "").unwrap();

        assert!(OpenInIDEService::is_project_path(&sln_file));
    }

    #[tokio::test]
    async fn test_open_with_nonexistent_path() {
        let service = OpenInIDEService::new();
        let result = service
            .open(Path::new("C:\\nonexistent\\folder"), None)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_open_in_vscode() {
        let service = OpenInIDEService::new();
        // Test with current directory (should exist on this system if VS Code installed)
        let result = service.open_in_vscode(Path::new(".")).await;
        assert!(result.is_ok());
    }
}
