pub mod linux;
pub mod macos;
pub mod folder_thumbnail;
pub mod windows;

#[cfg(target_os = "windows")]
pub use windows::desktop_ini_service;
#[cfg(target_os = "windows")]
pub use windows::shell;
#[cfg(target_os = "windows")]
pub use windows::shell_service;
#[cfg(target_os = "windows")]
pub use windows::wallpaper_service;
#[cfg(target_os = "windows")]
pub use windows::windows_interop;
#[cfg(target_os = "windows")]
pub use windows::wmi_service;
