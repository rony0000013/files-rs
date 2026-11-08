use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_INVOKEIDLIST, SHELLEXECUTEINFOW};

pub struct ShellService;

impl ShellService {
    /// Synchronous core of [`Self::open_properties`] (the body is sync
    /// ShellExecuteExW; the async wrapper exists for callers on async contexts).
    pub fn open_properties_sync(path: &FilesPath) -> Result<()> {
        Self::open_properties_with_hwnd(path, None)
    }

    /// Open properties targeting a specific parent HWND (as an isize handle)
    pub fn open_properties_with_hwnd(
        path: &FilesPath,
        hwnd: Option<isize>,
    ) -> Result<()> {
        Self::open_properties_page_with_hwnd(path, None, hwnd)
    }

    /// Open properties targeting a specific parent HWND and optional property sheet tab name (e.g. "Security").
    pub fn open_properties_page_with_hwnd(
        path: &FilesPath,
        page_name: Option<&str>,
        hwnd: Option<isize>,
    ) -> Result<()> {
        #[cfg(windows)]
        {
            let wide_path: Vec<u16> = path
                .as_path()
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let wide_page: Option<Vec<u16>> = page_name.map(|p| {
                p.encode_utf16().chain(std::iter::once(0)).collect()
            });

            let target_hwnd = hwnd
                .filter(|&h| h != 0)
                .map(|h| windows::Win32::Foundation::HWND(h as *mut _))
                .unwrap_or_else(|| unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() });

            let opt_hwnd = if target_hwnd.0.is_null() {
                None
            } else {
                Some(target_hwnd)
            };

            // 1. Prefer SHObjectProperties from Shell32 (SHOP_FILEPATH).
            // This directly invokes the Shell Property Sheet host for any file or folder path
            // without requiring or failing on file associations (which fails for folders with dots).
            let ret = unsafe {
                windows::Win32::UI::Shell::SHObjectProperties(
                    opt_hwnd,
                    windows::Win32::UI::Shell::SHOP_FILEPATH,
                    PCWSTR(wide_path.as_ptr()),
                    wide_page
                        .as_ref()
                        .map(|w| PCWSTR(w.as_ptr()))
                        .unwrap_or(PCWSTR::null()),
                )
            };

            if !ret.as_bool() {
                unsafe {
                    // Fallback to ShellExecuteExW with SEE_MASK_INVOKEIDLIST
                    let wide_op: Vec<u16> = "properties"
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();

                    let mut sei = SHELLEXECUTEINFOW::default();
                    sei.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
                    sei.fMask = SEE_MASK_INVOKEIDLIST;
                    sei.hwnd = target_hwnd;
                    sei.lpVerb = PCWSTR(wide_op.as_ptr());
                    sei.lpFile = PCWSTR(wide_path.as_ptr());
                    if let Some(ref wp) = wide_page {
                        sei.lpParameters = PCWSTR(wp.as_ptr());
                    }
                    sei.nShow = windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL.0;

                    let _ = ShellExecuteExW(&mut sei);
                }
            }
        }
        #[cfg(not(windows))]
        {
            let _ = (path, page_name, hwnd);
            tracing::warn!("open_properties is only implemented for Windows");
        }
        Ok(())
    }

    /// Open the native Windows Properties dialog for one or multiple selected files/folders.
    /// When multiple items are provided, uses `IShellFolder::GetUIObjectOf::<IDataObject>` + `SHMultiFileProperties`.
    pub fn open_multi_properties_with_hwnd(
        paths: &[FilesPath],
        hwnd: Option<isize>,
    ) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        if paths.len() == 1 {
            return Self::open_properties_with_hwnd(&paths[0], hwnd);
        }

        #[cfg(windows)]
        {
            use windows::Win32::System::Com::{
                CoInitializeEx, CoTaskMemFree, CoUninitialize, IDataObject,
                COINIT_APARTMENTTHREADED,
            };
            use windows::Win32::UI::Shell::Common::ITEMIDLIST;
            use windows::Win32::UI::Shell::{
                ILClone, ILFindLastID, IShellFolder, SHBindToParent, SHMultiFileProperties,
                SHParseDisplayName,
            };

            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                let mut opened = false;
                if let Some(first) = paths.first() {
                    if let Some(parent_dir) = first.as_path().parent() {
                        let dir_w: Vec<u16> = parent_dir
                            .as_os_str()
                            .encode_wide()
                            .chain(std::iter::once(0))
                            .collect();
                        let mut dir_pidl: *mut ITEMIDLIST = std::ptr::null_mut();
                        if SHParseDisplayName(PCWSTR(dir_w.as_ptr()), None, &mut dir_pidl, 0, None)
                            .is_ok()
                            && !dir_pidl.is_null()
                        {
                            let mut last_rel: *mut ITEMIDLIST = std::ptr::null_mut();
                            if let Ok(parent_sf) =
                                SHBindToParent::<IShellFolder>(dir_pidl, Some(&mut last_rel))
                            {
                                if let Ok(folder) =
                                    parent_sf.BindToObject::<_, IShellFolder>(last_rel, None)
                                {
                                    let mut child_pidls: Vec<*const ITEMIDLIST> = Vec::new();
                                    for p in paths {
                                        if p.as_path().parent() != Some(parent_dir) {
                                            continue;
                                        }
                                        let pw: Vec<u16> = p
                                            .as_path()
                                            .as_os_str()
                                            .encode_wide()
                                            .chain(std::iter::once(0))
                                            .collect();
                                        let mut full: *mut ITEMIDLIST = std::ptr::null_mut();
                                        if SHParseDisplayName(
                                            PCWSTR(pw.as_ptr()),
                                            None,
                                            &mut full,
                                            0,
                                            None,
                                        )
                                        .is_ok()
                                            && !full.is_null()
                                        {
                                            let rel = ILFindLastID(full);
                                            if !rel.is_null() {
                                                let owned = ILClone(rel);
                                                if !owned.is_null() {
                                                    child_pidls.push(owned);
                                                }
                                            }
                                            CoTaskMemFree(Some(full as *const _ as _));
                                        }
                                    }
                                    if !child_pidls.is_empty() {
                                        let target_hwnd = hwnd
                                            .filter(|&h| h != 0)
                                            .map(|h| windows::Win32::Foundation::HWND(h as *mut _))
                                            .unwrap_or_else(|| {
                                                windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow()
                                            });
                                        if let Ok(data_obj) = folder
                                            .GetUIObjectOf::<IDataObject>(
                                                target_hwnd,
                                                &child_pidls,
                                                None,
                                            )
                                        {
                                            opened = SHMultiFileProperties(&data_obj, 0).is_ok();
                                        }
                                    }
                                    for pidl in child_pidls {
                                        CoTaskMemFree(Some(pidl as *const _ as _));
                                    }
                                }
                            }
                            CoTaskMemFree(Some(dir_pidl as *const _ as _));
                        }
                    }
                }
                CoUninitialize();
                if !opened {
                    let _ = Self::open_properties_with_hwnd(&paths[0], hwnd);
                }
            }
        }
        #[cfg(not(windows))]
        {
            let _ = (paths, hwnd);
        }
        Ok(())
    }

    /// Open the native Windows "Open With" application picker dialog (`Change...` in General properties).
    pub fn open_with_dialog(path: &FilesPath, hwnd: Option<isize>) -> Result<()> {
        #[cfg(windows)]
        {
            let wide_file: Vec<u16> = "rundll32.exe"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let params = format!(
                "shell32.dll,OpenAs_RunDLL {}",
                path.as_path().to_string_lossy()
            );
            let wide_params: Vec<u16> = params
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let target_hwnd = hwnd
                .filter(|&h| h != 0)
                .map(|h| windows::Win32::Foundation::HWND(h as *mut _))
                .unwrap_or_default();

            unsafe {
                let mut sei = SHELLEXECUTEINFOW::default();
                sei.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
                sei.hwnd = target_hwnd;
                sei.lpFile = PCWSTR(wide_file.as_ptr());
                sei.lpParameters = PCWSTR(wide_params.as_ptr());
                sei.nShow = windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL.0;
                let _ = ShellExecuteExW(&mut sei);
            }
        }
        #[cfg(not(windows))]
        {
            let _ = (path, hwnd);
        }
        Ok(())
    }

    /// Launch Windows Disk Cleanup (`cleanmgr.exe`) for a given drive root (e.g. `C:\`).
    pub fn cleanup_drive(drive_root: &str) -> Result<()> {
        #[cfg(windows)]
        {
            let letter = drive_root.chars().next().unwrap_or('C');
            let _ = std::process::Command::new("cleanmgr.exe")
                .args(["/d", &format!("{}:", letter)])
                .spawn();
        }
        #[cfg(not(windows))]
        {
            let _ = drive_root;
        }
        Ok(())
    }

    pub async fn open_properties(path: &FilesPath) -> Result<()> {
        Self::open_properties_sync(path)
    }

    pub async fn format_drive(drive_letter: &str) -> Result<()> {
        #[cfg(windows)]
        {
            let wide_drive: Vec<u16> = format!("{}:", drive_letter)
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let wide_op: Vec<u16> = "format".encode_utf16().chain(std::iter::once(0)).collect();

            let mut sei = SHELLEXECUTEINFOW::default();
            sei.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
            sei.fMask = SEE_MASK_INVOKEIDLIST;
            sei.lpVerb = PCWSTR(wide_op.as_ptr());
            sei.lpFile = PCWSTR(wide_drive.as_ptr());
            sei.nShow = windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL.0;

            unsafe {
                ShellExecuteExW(&mut sei)
                    .map_err(|e| anyhow!("Failed to open format dialog: {}", e))?;
            }
        }
        #[cfg(not(windows))]
        {
            tracing::warn!("format_drive is only implemented for Windows");
        }
        Ok(())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[tokio::test]
    #[cfg(not(windows))]
    async fn test_open_properties_creates_no_error() {
        // On non-Windows, this just logs a warning and returns Ok
        let path = FilesPath::new("C:\\test\\nonexistent.txt");
        let result = ShellService::open_properties(&path).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[cfg(not(windows))]
    async fn test_format_drive_creates_no_error() {
        // This only logs on non-Windows, returns Ok
        // On Windows, this can actually format drives so we skip it
        let result = ShellService::format_drive("C").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[cfg(not(windows))]
    async fn test_open_properties_with_different_paths() {
        // On non-Windows, these just log warnings
        let paths = vec![
            "C:\\Windows\\System32",
            "D:\\Users\\Public",
            "C:\\Program Files",
        ];

        for path_str in paths {
            let path = FilesPath::new(path_str);
            let result = ShellService::open_properties(&path).await;
            assert!(result.is_ok(), "Failed for path: {}", path_str);
        }
    }

    #[tokio::test]
    #[cfg(not(windows))]
    async fn test_format_drive_various_letters() {
        // This only logs on non-Windows
        // On Windows, format can cause data loss so we skip it
        for letter in ['C', 'D', 'X', 'Z'] {
            let result = ShellService::format_drive(&letter.to_string()).await;
            assert!(result.is_ok(), "Failed for drive: {}", letter);
        }
    }
}
