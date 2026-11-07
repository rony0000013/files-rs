//! Native Windows classic context menu ("Show more options").
//!
//! Hosts `IContextMenu` for the selected items (or the current folder
//! background) on a dedicated STA thread, so every third-party shell
//! extension (WinRAR, 7-Zip, Notepad++, ...) and the full classic verb set
//! appear exactly like in File Explorer / Files-community's overflow menu.
//! This is the extension point other apps plug into on Windows; other
//! platforms get a graceful no-op (their native provider can fill the same
//! "Show more options" row later — the menu rows are already data-driven).

use std::path::{Path, PathBuf};

/// Show the native classic shell menu for `paths` (empty = folder
/// background of `dir`) at screen position (`x`, `y`). Runs on a dedicated
/// STA thread; returns after the user picks a command or dismisses the menu.
pub fn show_classic_menu(paths: Vec<PathBuf>, dir: PathBuf, x: i32, y: i32) {
    #[cfg(windows)]
    {
        std::thread::spawn(move || {
            let _ = show_classic_menu_impl(&paths, &dir, x, y);
        });
    }
    #[cfg(not(windows))]
    {
        let _ = (paths, dir, x, y);
        tracing::warn!("Classic shell menu is only implemented on Windows");
    }
}

#[cfg(windows)]
fn show_classic_menu_impl(paths: &[PathBuf], dir: &Path, x: i32, y: i32) -> anyhow::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, POINT};
    use windows::Win32::System::Com::{
        CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED,
        COINIT_DISABLE_OLE1DDE,
    };
    use windows::Win32::UI::Shell::{
        CMF_EXTENDEDVERBS, CMF_NORMAL, CMINVOKECOMMANDINFO, IContextMenu, ILClone, ILFindLastID,
        IShellFolder, SHBindToParent, SHParseDisplayName,
    };
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreatePopupMenu, DestroyMenu, GetCursorPos, GetForegroundWindow, TrackPopupMenuEx,
        TPM_LEFTALIGN, TPM_RETURNCMD,
    };

    unsafe {
        // IContextMenu must live on an STA thread.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);

        let result = (|| -> anyhow::Result<()> {
            // Bind the containing folder: parse dir -> parent + relative PIDL
            // -> bind the relative PIDL to the folder object itself.
            let dir_w: Vec<u16> = dir
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let mut dir_pidl: *mut ITEMIDLIST = std::ptr::null_mut();
            SHParseDisplayName(
                PCWSTR(dir_w.as_ptr()),
                None,
                &mut dir_pidl,
                0,
                None,
            )
            .map_err(|e| anyhow::anyhow!("parse dir: {e}"))?;
            if dir_pidl.is_null() {
                anyhow::bail!("parse dir returned null PIDL");
            }

            let mut last_rel: *mut ITEMIDLIST = std::ptr::null_mut();
            let parent: IShellFolder = SHBindToParent(dir_pidl, Some(&mut last_rel))
                .map_err(|e| anyhow::anyhow!("bind parent: {e}"))?;
            let folder: IShellFolder = parent
                .BindToObject(last_rel, None)
                .map_err(|e| anyhow::anyhow!("bind folder: {e}"))?;

            // Collect child PIDLs (empty = background menu of the folder).
            // Selections always live in one dir; drop strays by path first.
            let mut child_pidls: Vec<*const ITEMIDLIST> = Vec::new();
            for p in paths {
                if p.parent() != Some(dir) {
                    continue;
                }
                let pw: Vec<u16> = p
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
                .is_err()
                || full.is_null()
                {
                    continue;
                }
                // ILFindLastID returns an interior pointer into `full`, so it
                // must be cloned (ILClone) before `full` is freed — using it
                // after CoTaskMemFree(full) is use-after-free and corrupts the
                // heap (this crashed the app).
                let rel = ILFindLastID(full);
                let owned = if rel.is_null() {
                    std::ptr::null_mut()
                } else {
                    ILClone(rel)
                };
                CoTaskMemFree(Some(full as *const _ as _));
                if !owned.is_null() {
                    child_pidls.push(owned);
                }
            }

            let hwnd: HWND = GetForegroundWindow();
            let cm: IContextMenu = if child_pidls.is_empty() {
                parent
                    .GetUIObjectOf(hwnd, &[last_rel], None)
                    .map_err(|e| anyhow::anyhow!("GetUIObjectOf dir: {e}"))?
            } else {
                folder
                    .GetUIObjectOf(hwnd, &child_pidls, None)
                    .map_err(|e| anyhow::anyhow!("GetUIObjectOf: {e}"))?
            };

            let hmenu =
                CreatePopupMenu().map_err(|e| anyhow::anyhow!("CreatePopupMenu: {e}"))?;
            // ID range 1..=0x6FFF for shell commands.
            let hr = cm.QueryContextMenu(hmenu, 0, 1, 0x6FFF, CMF_NORMAL | CMF_EXTENDEDVERBS);
            if hr.0 < 0 {
                let _ = DestroyMenu(hmenu);
                anyhow::bail!("QueryContextMenu failed");
            }

            // Caller passes screen coords; fall back to the cursor.
            let (px, py) = if x == i32::MIN && y == i32::MIN {
                let mut pt = POINT { x: 0, y: 0 };
                GetCursorPos(&mut pt).ok();
                (pt.x, pt.y)
            } else {
                (x, y)
            };

            let cmd = TrackPopupMenuEx(
                hmenu,
                TPM_LEFTALIGN.0 | TPM_RETURNCMD.0,
                px,
                py,
                hwnd,
                None,
            );
            let _ = DestroyMenu(hmenu);
            for pidl in child_pidls {
                CoTaskMemFree(Some(pidl as *const _ as _));
            }
            CoTaskMemFree(Some(dir_pidl as *const _ as _));

            if cmd.0 != 0 {
                // IDs were allocated from 1, so the offset is cmd - 1.
                let verb = (cmd.0 - 1) as usize;
                let info = CMINVOKECOMMANDINFO {
                    cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
                    hwnd,
                    lpVerb: windows::core::PCSTR(verb as *const u8),
                    nShow: 1, // SW_SHOWNORMAL
                    ..Default::default()
                };
                cm.InvokeCommand(&info)
                    .map_err(|e| anyhow::anyhow!("InvokeCommand: {e}"))?;
            }
            Ok(())
        })();

        CoUninitialize();
        if let Err(e) = result {
            tracing::warn!("Classic shell menu failed: {:#}", e);
        }
        Ok(())
    }
}
