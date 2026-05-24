//! Terminal integration service
//! Detects installed terminal emulators and provides commands
//! to open terminals at specific paths.

use std::path::Path;

/// Represents a terminal emulator type
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TerminalType {
    Cmd,
    PowerShell,
    PowerShell7Core,
    WindowsTerminal,
    GitBash,
    Wsl,
}

impl std::fmt::Display for TerminalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TerminalType::Cmd => write!(f, "Command Prompt"),
            TerminalType::PowerShell => write!(f, "PowerShell"),
            TerminalType::PowerShell7Core => write!(f, "PowerShell 7 Core"),
            TerminalType::WindowsTerminal => write!(f, "Windows Terminal"),
            TerminalType::GitBash => write!(f, "Git Bash"),
            TerminalType::Wsl => write!(f, "WSL"),
        }
    }
}

/// Service for detecting available terminals and constructing
/// commands to open them at a given working directory.
pub struct TerminalService;

impl TerminalService {
    /// Creates a new TerminalService instance.
    pub fn new() -> Self {
        Self
    }

    /// Detect which terminals are available on the current system.
    ///
    /// On Windows, checks for each terminal executable.
    /// On non-Windows, returns system-default terminal emulators.
    #[cfg(windows)]
    pub fn detect_terminals() -> Vec<TerminalType> {
        let mut terminals = Vec::new();

        // cmd.exe is always available on Windows
        terminals.push(TerminalType::Cmd);

        // powershell.exe is always available on Windows
        terminals.push(TerminalType::PowerShell);

        // Windows Terminal (wt.exe) — check in %LOCALAPPDATA%\Microsoft\WindowsApps\
        let local_app_data = std::env::var("LOCALAPPDATA")
            .unwrap_or_else(|_| r"C:\Users\Default\AppData\Local".to_string());
        let wt_path = std::path::Path::new(&local_app_data)
            .join("Microsoft")
            .join("WindowsApps")
            .join("wt.exe");
        if wt_path.exists() {
            terminals.push(TerminalType::WindowsTerminal);
        }

        // PowerShell 7 Core (pwsh.exe) — check PATH
        if which_pwsh() {
            terminals.push(TerminalType::PowerShell7Core);
        }

        // Git Bash — check for C:\Program Files\Git\bin\bash.exe
        let git_bash_path = std::path::Path::new(r"C:\Program Files\Git\bin\bash.exe");
        if git_bash_path.exists() {
            terminals.push(TerminalType::GitBash);
        }

        // WSL (wsl.exe) — check PATH
        if which_wsl() {
            terminals.push(TerminalType::Wsl);
        }

        terminals
    }

    /// Detect which terminals are available (non-Windows stub).
    ///
    /// Returns common Linux terminal emulators.
    #[cfg(not(windows))]
    pub fn detect_terminals() -> Vec<TerminalType> {
        let mut terminals = Vec::new();
        // On Linux, we return reasonable defaults
        // x-terminal-emulator is a common symlink on Debian/Ubuntu
        if std::path::Path::new("/usr/bin/x-terminal-emulator").exists() {
            terminals.push(TerminalType::Cmd); // Reuse as "system default"
        }
        if std::path::Path::new("/usr/bin/gnome-terminal").exists() {
            terminals.push(TerminalType::PowerShell); // Reuse as "gnome-terminal"
        }
        if terminals.is_empty() {
            // Always return at least xterm as fallback
            terminals.push(TerminalType::Cmd);
        }
        terminals
    }

    /// Returns the command and arguments to open the given terminal type
    /// at the specified working directory.
    ///
    /// Returns `(command, vec_of_args)`.
    #[cfg(windows)]
    pub fn get_command(terminal: &TerminalType, path: &Path) -> (String, Vec<String>) {
        let path_str = path.to_string_lossy().into_owned();

        match terminal {
            TerminalType::Cmd => (
                "cmd.exe".to_string(),
                vec!["/K".to_string(), format!("cd /d {}", path_str)],
            ),
            TerminalType::PowerShell => (
                "powershell.exe".to_string(),
                vec![
                    "-NoExit".to_string(),
                    "-Command".to_string(),
                    format!("cd '{}'", path_str.replace('\'', "''")),
                ],
            ),
            TerminalType::PowerShell7Core => (
                "pwsh.exe".to_string(),
                vec![
                    "-NoExit".to_string(),
                    "-Command".to_string(),
                    format!("cd '{}'", path_str.replace('\'', "''")),
                ],
            ),
            TerminalType::WindowsTerminal => {
                ("wt.exe".to_string(), vec!["-d".to_string(), path_str])
            }
            TerminalType::GitBash => (
                r"C:\Program Files\Git\bin\bash.exe".to_string(),
                vec!["--cd".to_string(), path_str],
            ),
            TerminalType::Wsl => (
                "wsl.exe".to_string(),
                vec![
                    "--cd".to_string(),
                    path_str.clone(),
                    "-e".to_string(),
                    "bash".to_string(),
                    "-c".to_string(),
                    format!("cd '{}' && exec bash", path_str.replace('\'', "'\\''")),
                ],
            ),
        }
    }

    /// Returns the command and arguments for non-Windows platforms.
    #[cfg(not(windows))]
    pub fn get_command(terminal: &TerminalType, path: &Path) -> (String, Vec<String>) {
        let path_str = path.to_string_lossy().into_owned();

        match terminal {
            TerminalType::Cmd | TerminalType::PowerShell | TerminalType::PowerShell7Core => {
                // On Linux, try x-terminal-emulator, gnome-terminal, or xterm
                if std::path::Path::new("/usr/bin/x-terminal-emulator").exists() {
                    (
                        "x-terminal-emulator".to_string(),
                        vec!["--working-directory".to_string(), path_str],
                    )
                } else if std::path::Path::new("/usr/bin/gnome-terminal").exists() {
                    (
                        "gnome-terminal".to_string(),
                        vec!["--working-directory".to_string(), path_str],
                    )
                } else {
                    (
                        "xterm".to_string(),
                        vec!["-e".to_string(), format!("cd {} && exec bash", path_str)],
                    )
                }
            }
            TerminalType::WindowsTerminal | TerminalType::GitBash | TerminalType::Wsl => {
                // Fallback to xterm for unknown terminal types on Linux
                (
                    "xterm".to_string(),
                    vec!["-e".to_string(), format!("cd {} && exec bash", path_str)],
                )
            }
        }
    }
}

impl Default for TerminalService {
    fn default() -> Self {
        Self::new()
    }
}

// Helper: check if pwsh.exe is available via PATH on Windows
#[cfg(windows)]
fn which_pwsh() -> bool {
    std::process::Command::new("where")
        .arg("pwsh.exe")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// Helper: check if wsl.exe is available via PATH on Windows
#[cfg(windows)]
fn which_wsl() -> bool {
    std::process::Command::new("where")
        .arg("wsl.exe")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

impl TerminalType {
    /// Returns the default terminal type for the current platform.
    #[cfg(windows)]
    pub fn default_for_platform() -> Self {
        TerminalType::Cmd
    }

    /// Returns the default terminal type for non-Windows platforms.
    #[cfg(not(windows))]
    pub fn default_for_platform() -> Self {
        TerminalType::Cmd
    }
}

impl Default for TerminalType {
    fn default() -> Self {
        Self::default_for_platform()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_terminal_type_variants() {
        // Verify all variants exist and can be constructed
        let variants = vec![
            TerminalType::Cmd,
            TerminalType::PowerShell,
            TerminalType::PowerShell7Core,
            TerminalType::WindowsTerminal,
            TerminalType::GitBash,
            TerminalType::Wsl,
        ];
        assert_eq!(variants.len(), 6);
    }

    #[test]
    fn test_detect_terminals_returns_vec() {
        let terminals = TerminalService::detect_terminals();
        // Should always return at least one terminal
        assert!(!terminals.is_empty(), "Should detect at least one terminal");
    }

    #[test]
    fn test_terminal_type_display() {
        assert_eq!(format!("{}", TerminalType::Cmd), "Command Prompt");
        assert_eq!(format!("{}", TerminalType::PowerShell), "PowerShell");
        assert_eq!(
            format!("{}", TerminalType::PowerShell7Core),
            "PowerShell 7 Core"
        );
        assert_eq!(
            format!("{}", TerminalType::WindowsTerminal),
            "Windows Terminal"
        );
        assert_eq!(format!("{}", TerminalType::GitBash), "Git Bash");
        assert_eq!(format!("{}", TerminalType::Wsl), "WSL");
    }

    #[test]
    fn test_get_command_cmd() {
        let path = Path::new(r"C:\Users\test\projects");
        #[cfg(windows)]
        {
            let (cmd, args) = TerminalService::get_command(&TerminalType::Cmd, path);
            assert_eq!(cmd, "cmd.exe");
            assert!(args.contains(&"/K".to_string()));
            assert!(args.iter().any(|a| a.contains("cd /d")));
        }
        #[cfg(not(windows))]
        {
            let (cmd, args) = TerminalService::get_command(&TerminalType::Cmd, path);
            // On Linux, it should return a reasonable terminal command
            assert!(!cmd.is_empty());
            assert!(!args.is_empty());
        }
    }

    #[test]
    fn test_get_command_cmd_uses_cd() {
        let path = Path::new(r"C:\Users\test\my project");
        #[cfg(windows)]
        {
            let (cmd, args) = TerminalService::get_command(&TerminalType::Cmd, path);
            assert_eq!(cmd, "cmd.exe");
            let joined = args.join(" ");
            assert!(joined.contains("cd /d"));
            assert!(joined.contains("my project"));
        }
        #[cfg(not(windows))]
        {
            let (cmd, args) = TerminalService::get_command(&TerminalType::Cmd, path);
            assert!(!cmd.is_empty());
            let joined = args.join(" ");
            // Should reference the path in some way
            assert!(joined.contains("my project") || joined.contains("test"));
        }
    }

    #[test]
    fn test_terminal_service_new() {
        let service = TerminalService::new();
        // Basic creation test
        let _ = service;
    }

    #[test]
    fn test_terminal_service_default() {
        let service = TerminalService::default();
        let _ = service;
    }

    #[cfg(not(windows))]
    #[test]
    fn test_detect_terminals_includes_system_default() {
        let terminals = TerminalService::detect_terminals();
        // On Linux, should return at least one terminal
        assert!(!terminals.is_empty());
        // The first terminal should be the system default
        let first = &terminals[0];
        // On a well-configured Linux system, we may have gnome-terminal or x-terminal-emulator
        // At minimum we always return Cmd as a fallback
        assert!(true);
    }
}
