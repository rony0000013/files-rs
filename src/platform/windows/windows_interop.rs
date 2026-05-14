use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow};

pub fn focus_existing_window(window_title: &str) -> Result<(), String> {
    let title: Vec<u16> = window_title
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let pcwstr = PCWSTR(title.as_ptr());

    unsafe {
        let hwnd = FindWindowW(None, pcwstr);
        if hwnd == HWND(0) {
            return Err("Could not find existing window".to_string());
        }
        if !SetForegroundWindow(hwnd).as_bool() {
            return Err("Failed to set foreground window".to_string());
        }
    }
    Ok(())
}

pub struct ComThread {
    tx: std::sync::mpsc::Sender<ComTask>,
    _handle: std::thread::JoinHandle<()>,
}

pub enum ComTask {
    Execute(Box<dyn FnOnce() + Send>),
}

impl ComThread {
    pub fn new() -> anyhow::Result<Self> {
        let (tx, rx) = std::sync::mpsc::channel::<ComTask>();

        let handle = std::thread::spawn(move || {
            println!("COM STA Thread started.");

            while let Ok(task) = rx.recv() {
                match task {
                    ComTask::Execute(f) => {
                        f();
                    }
                }
            }

            println!("COM STA Thread shutting down.");
        });

        Ok(Self {
            tx,
            _handle: handle,
        })
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let _ = self.tx.send(ComTask::Execute(Box::new(f)));
    }
}
pub fn windows_build_number() -> u32 {
    use windows::Win32::System::SystemInformation::{GetVersionExW, OSVERSIONINFOW};
    unsafe {
        let mut info: OSVERSIONINFOW = std::mem::zeroed();
        info.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOW>() as u32;
        if GetVersionExW(&mut info).is_ok() {
            info.dwBuildNumber
        } else {
            0
        }
    }
}

pub fn system_font() -> String {
    if windows_build_number() >= 22000 {
        "Segoe UI Variable Text".to_string()
    } else {
        "Segoe UI".to_string()
    }
}

pub fn read_accent_color() -> gpui::Rgba {
    #[cfg(windows)]
    {
        use winreg::{enums::HKEY_CURRENT_USER, RegKey};
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(key) = hkcu.open_subkey("Software\\Microsoft\\Windows\\DWM") {
            if let Ok(abgr) = key.get_value::<u32, _>("AccentColor") {
                // ABGR -> RGBA
                let r = (abgr & 0xFF) as f32 / 255.0;
                let g = ((abgr >> 8) & 0xFF) as f32 / 255.0;
                let b = ((abgr >> 16) & 0xFF) as f32 / 255.0;
                let a = ((abgr >> 24) & 0xFF) as f32 / 255.0;
                return gpui::Rgba { r, g, b, a };
            }
        }
    }
    // Default Windows 11 Blue: #0078D4
    gpui::Rgba {
        r: 0.0,
        g: 0.47,
        b: 0.83,
        a: 1.0,
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_system_font_win11() {
        // Test that system_font returns a string (either Segoe UI or Segoe UI Variable)
        let font = system_font();
        assert!(font.contains("Segoe UI"));
    }

    #[test]
    fn test_focus_existing_window_invalid_title() {
        // Test that invalid window title returns error
        let result = focus_existing_window("NonExistentWindowTitleXYZ123");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Could not find"));
    }

    #[test]
    fn test_com_task_execute_box() {
        // Test that ComTask::Execute can hold a boxed closure
        let task = ComTask::Execute(Box::new(|| {
            println!("Test closure executed");
        }));
        match task {
            ComTask::Execute(f) => {
                f();
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_build_number() {
        let build = windows_build_number();
        assert!(build > 0);
    }

    #[cfg(not(windows))]
    #[test]
    fn test_windows_build_number_non_windows() {
        let build = windows_build_number();
        assert_eq!(build, 0);
    }

    #[cfg(windows)]
    #[test]
    fn test_read_accent_color() {
        let color = read_accent_color();
        // Check that color values are in valid range
        assert!(color.r >= 0.0 && color.r <= 1.0);
        assert!(color.g >= 0.0 && color.g <= 1.0);
        assert!(color.b >= 0.0 && color.b <= 1.0);
        assert!(color.a >= 0.0 && color.a <= 1.0);
    }

    #[cfg(windows)]
    #[test]
    fn test_com_thread_creation() {
        let result = ComThread::new();
        assert!(result.is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn test_com_thread_execute() {
        use std::sync::Arc;

        let com_thread = ComThread::new().unwrap();
        let executed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let executed_clone = executed.clone();

        com_thread.execute(move || {
            executed_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        // Give some time for the task to be executed
        std::thread::sleep(std::time::Duration::from_millis(100));

        assert!(executed.load(std::sync::atomic::Ordering::SeqCst));
    }
}
