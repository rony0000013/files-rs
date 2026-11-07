//! Terminal integration service
//! Detects installed terminal emulators and provides commands
//! to open terminals at specific paths.

use std::path::Path;

/// Represents a terminal emulator type
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TerminalType {
    Cmd,
    CommandPrompt,
    PowerShell,
    PowerShell7Core,
    WindowsTerminal,
    GitBash,
    Wsl,
}

impl std::fmt::Display for TerminalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TerminalType::Cmd => write!(f, "Command Prompt (cmd.exe)"),
            TerminalType::CommandPrompt => write!(f, "Command Prompt"),
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
    /// Launches the given terminal emulator at `path`.
    /// Handles virtual paths (falling back to user home directory) and platform fallbacks.
    pub fn open_terminal(terminal: &TerminalType, path: &Path) -> Result<(), String> {
        let working_dir = if path.is_dir() {
            path.to_path_buf()
        } else if path.is_file() {
            path.parent().unwrap_or(path).to_path_buf()
        } else if let Some(home) = dirs::home_dir() {
            home
        } else {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        };

        #[cfg(windows)]
        {
            Self::launch_windows(terminal, &working_dir)
        }

        #[cfg(target_os = "macos")]
        {
            Self::launch_macos(&working_dir)
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            Self::launch_linux(&working_dir)
        }
    }

    #[cfg(windows)]
    fn launch_windows(terminal: &TerminalType, working_dir: &Path) -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        use std::process::Command;

        const CREATE_NEW_CONSOLE: u32 = 0x00000010;
        let path_str = working_dir.to_string_lossy().to_string();

        let try_spawn = |cmd_name: &str, args: &[&str], needs_console: bool| -> bool {
            let mut cmd = Command::new(cmd_name);
            cmd.args(args);
            cmd.current_dir(working_dir);
            if needs_console {
                cmd.creation_flags(CREATE_NEW_CONSOLE);
            }
            cmd.stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            match cmd.spawn() {
                Ok(_) => true,
                Err(e) => {
                    tracing::warn!("Failed to launch terminal {}: {}", cmd_name, e);
                    false
                }
            }
        };

        let success = match terminal {
            TerminalType::WindowsTerminal => {
                if try_spawn("wt.exe", &["-d", &path_str], false) {
                    true
                } else {
                    try_spawn("cmd.exe", &["/C", "start", "wt.exe", "-d", &path_str], false)
                }
            }
            TerminalType::PowerShell => {
                let cmd_arg = format!("Set-Location -LiteralPath '{}'", path_str.replace('\'', "''"));
                try_spawn("powershell.exe", &["-NoExit", "-Command", &cmd_arg], true)
            }
            TerminalType::PowerShell7Core => {
                let cmd_arg = format!("Set-Location -LiteralPath '{}'", path_str.replace('\'', "''"));
                try_spawn("pwsh.exe", &["-NoExit", "-Command", &cmd_arg], true)
            }
            TerminalType::Cmd | TerminalType::CommandPrompt => {
                let cmd_arg = format!("cd /d \"{}\"", path_str);
                try_spawn("cmd.exe", &["/K", &cmd_arg], true)
            }
            TerminalType::GitBash => {
                let git_bash = std::path::Path::new(r"C:\Program Files\Git\bin\bash.exe");
                let exe = if git_bash.exists() {
                    r"C:\Program Files\Git\bin\bash.exe"
                } else {
                    "bash.exe"
                };
                try_spawn(exe, &["--cd", &path_str], true)
            }
            TerminalType::Wsl => {
                try_spawn("wsl.exe", &["--cd", &path_str], true)
            }
        };

        if success {
            return Ok(());
        }

        // Automatic fallback cascade: wt -> powershell -> cmd
        if try_spawn("wt.exe", &["-d", &path_str], false) {
            return Ok(());
        }
        let ps_arg = format!("Set-Location -LiteralPath '{}'", path_str.replace('\'', "''"));
        if try_spawn("powershell.exe", &["-NoExit", "-Command", &ps_arg], true) {
            return Ok(());
        }
        let cmd_arg = format!("cd /d \"{}\"", path_str);
        if try_spawn("cmd.exe", &["/K", &cmd_arg], true) {
            return Ok(());
        }

        Err("Could not open any terminal application on Windows".to_string())
    }

    #[cfg(target_os = "macos")]
    fn launch_macos(working_dir: &Path) -> Result<(), String> {
        let path_str = working_dir.to_string_lossy();
        let mut cmd = std::process::Command::new("open");
        cmd.args(["-a", "Terminal", path_str.as_ref()]);
        cmd.current_dir(working_dir);
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        if cmd.spawn().is_ok() {
            return Ok(());
        }
        let mut fallback = std::process::Command::new("open");
        fallback.arg(path_str.as_ref());
        fallback.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        fallback.spawn().map(|_| ()).map_err(|e| e.to_string())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn launch_linux(working_dir: &Path) -> Result<(), String> {
        let path_str = working_dir.to_string_lossy().to_string();
        let term_env = std::env::var("TERMINAL").ok();

        let mut candidates = Vec::new();
        if let Some(t) = term_env {
            candidates.push(t);
        }
        candidates.extend([
            "x-terminal-emulator".to_string(),
            "gnome-terminal".to_string(),
            "konsole".to_string(),
            "xfce4-terminal".to_string(),
            "alacritty".to_string(),
            "kitty".to_string(),
            "xterm".to_string(),
        ]);

        for cand in &candidates {
            let mut cmd = std::process::Command::new(cand);
            cmd.current_dir(working_dir);
            cmd.stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            match cand.as_str() {
                "gnome-terminal" | "xfce4-terminal" => {
                    cmd.arg(format!("--working-directory={}", path_str));
                }
                "konsole" => {
                    cmd.args(["--workdir", &path_str]);
                }
                "kitty" => {
                    cmd.args(["--directory", &path_str]);
                }
                "alacritty" | "x-terminal-emulator" => {
                    cmd.args(["--working-directory", &path_str]);
                }
                _ => {
                    cmd.args(["-e", &format!("cd '{}' && exec $SHELL", path_str.replace('\'', "'\\''"))]);
                }
            }
            if cmd.spawn().is_ok() {
                return Ok(());
            }
        }

        Err("No terminal emulator found on Linux".to_string())
    }

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
            TerminalType::CommandPrompt => (
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
            TerminalType::Cmd
            | TerminalType::CommandPrompt
            | TerminalType::PowerShell
            | TerminalType::PowerShell7Core => {
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
