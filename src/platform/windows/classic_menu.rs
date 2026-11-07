//! Inline classic shell-menu items ("Show more options" flyout).
//!
//! Files-community parity: the classic items expand INLINE as a hover flyout
//! (like Files), not as a separate native popup. The native `IContextMenu`
//! is queried into an `HMENU`, enumerated here to plain data (labels,
//! enabled state, real extension icons), rendered as GPUI rows by
//! `dialogs_context`, and clicks are routed back through `InvokeCommand`.
//! This is the extension point other apps plug into on Windows — WinRAR,
//! 7-Zip, Kaspersky, PowerToys, ... all appear with their real icons.
//! Other platforms get an empty list (their provider can fill the same
//! "Show more options" row later — the menu rows are already data-driven).

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One classic shell-menu item, safe to hold on the UI thread (no COM handles).
#[derive(Clone, PartialEq, Debug)]
pub struct ClassicMenuEntry {
    /// Display label (separators have an empty label + `separator == true`).
    pub label: String,
    /// Command offset (`wID - idCmdFirst`) for `InvokeCommand`. Only valid
    /// for non-separator entries from the same query.
    pub offset: u32,
    /// False for grayed/disabled items (render dimmed, ignore clicks).
    pub enabled: bool,
    /// True for separator lines.
    pub separator: bool,
    /// Real extension icon, converted to BGRA bytes for `RenderImage`.
    pub icon: Option<Arc<gpui::RenderImage>>,
    /// Nested submenu (e.g. WinRAR ›) — same id space, invoke directly.
    pub children: Vec<ClassicMenuEntry>,
}

/// Query the classic menu for `paths` (empty = folder background of `dir`)
/// and return its items for inline rendering. Runs COM on the calling
/// thread — call from a background task, never the UI thread. Returns an
/// empty vec on non-Windows or on any shell failure.
pub fn enumerate_classic_menu(paths: &[PathBuf], dir: &Path) -> Vec<ClassicMenuEntry> {
    enumerate_with_icons(paths, dir, true)
}

/// Labels/states/separators only (no GetDIBits icon extraction) — the fast
/// first phase so the flyout can render instantly; icons fill in after.
pub fn enumerate_classic_labels(paths: &[PathBuf], dir: &Path) -> Vec<ClassicMenuEntry> {
    enumerate_with_icons(paths, dir, false)
}

#[allow(clippy::unused_variables)]
fn enumerate_with_icons(paths: &[PathBuf], dir: &Path, load_icons: bool) -> Vec<ClassicMenuEntry> {
    #[cfg(windows)]
    {
        enumerate_classic_menu_impl(paths, dir, load_icons).unwrap_or_default()
    }
    #[cfg(not(windows))]
    {
        let _ = (paths, dir, load_icons);
        Vec::new()
    }
}

/// Invoke a classic verb previously returned by `enumerate_classic_menu`.
/// Re-queries the menu and matches by (offset, label) so a stale offset can
/// never fire the wrong command. Runs COM on the calling thread — call from
/// a background task.
pub fn invoke_classic_verb(
    paths: &[PathBuf],
    dir: &Path,
    offset: u32,
    expect_label: &str,
) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        invoke_classic_verb_impl(paths, dir, offset, expect_label)
    }
    #[cfg(not(windows))]
    {
        let _ = (paths, dir, offset, expect_label);
        anyhow::bail!("Classic shell menu is only implemented on Windows");
    }
}

#[cfg(windows)]
fn with_sta<T>(f: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    use windows::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };
    unsafe {
        // No awaits may happen between init/uninit: the whole closure runs
        // on this one thread, so STA affinity holds.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        let result = f();
        CoUninitialize();
        result
    }
}

/// Bind the folder for `dir` and query its `IContextMenu` into a fresh
/// `HMENU`. Caller must `DestroyMenu` the handle. Child PIDLs are owned
/// clones (`ILClone`) — never use `ILFindLastID` results past the free.
#[cfg(windows)]
fn query_shell_menu(
    paths: &[PathBuf],
    dir: &Path,
) -> anyhow::Result<(
    windows::Win32::UI::Shell::IContextMenu,
    windows::Win32::UI::WindowsAndMessaging::HMENU,
)> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::{
        CMF_NORMAL, IContextMenu, ILClone, ILFindLastID, IShellFolder, SHBindToParent,
        SHParseDisplayName,
    };
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::WindowsAndMessaging::{CreatePopupMenu, GetForegroundWindow};

    unsafe {
        // Bind the containing folder: parse dir -> parent + relative PIDL
        // -> bind the relative PIDL to the folder object itself.
        let dir_w: Vec<u16> = dir
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut dir_pidl: *mut ITEMIDLIST = std::ptr::null_mut();
        SHParseDisplayName(PCWSTR(dir_w.as_ptr()), None, &mut dir_pidl, 0, None)
            .map_err(|e| anyhow::anyhow!("parse dir: {e}"))?;
        if dir_pidl.is_null() {
            anyhow::bail!("parse dir returned null PIDL");
        }

        let result = (|| -> anyhow::Result<(IContextMenu, _)> {
            let mut last_rel: *mut ITEMIDLIST = std::ptr::null_mut();
            let parent: IShellFolder = SHBindToParent(dir_pidl, Some(&mut last_rel))
                .map_err(|e| anyhow::anyhow!("bind parent: {e}"))?;
            // `last_rel` is interior to `dir_pidl`; it is only used here
            // while `dir_pidl` is still alive.
            let folder: IShellFolder = parent
                .BindToObject(last_rel, None)
                .map_err(|e| anyhow::anyhow!("bind folder: {e}"))?;

            // Collect owned child PIDLs (empty = background menu).
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
                if SHParseDisplayName(PCWSTR(pw.as_ptr()), None, &mut full, 0, None).is_err()
                    || full.is_null()
                {
                    continue;
                }
                // ILFindLastID is interior into `full`: clone before freeing.
                let rel = ILFindLastID(full);
                let owned = if rel.is_null() {
                    std::ptr::null_mut()
                } else {
                    ILClone(rel)
                };
                windows::Win32::System::Com::CoTaskMemFree(Some(full as *const _ as _));
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
            for pidl in child_pidls {
                windows::Win32::System::Com::CoTaskMemFree(Some(pidl as *const _ as _));
            }

            let hmenu =
                CreatePopupMenu().map_err(|e| anyhow::anyhow!("CreatePopupMenu: {e}"))?;
            // IDs 1..=0x6FFF; offsets (wID - 1) are stable per query.
            let hr = cm.QueryContextMenu(hmenu, 0, 1, 0x6FFF, CMF_NORMAL);
            if hr.0 < 0 {
                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyMenu(hmenu);
                anyhow::bail!("QueryContextMenu failed");
            }
            Ok((cm, hmenu))
        })();

        windows::Win32::System::Com::CoTaskMemFree(Some(dir_pidl as *const _ as _));
        result
    }
}

#[cfg(windows)]
fn enumerate_classic_menu_impl(
    paths: &[PathBuf],
    dir: &Path,
    load_icons: bool,
) -> anyhow::Result<Vec<ClassicMenuEntry>> {
    with_sta(|| {
        let (cm, hmenu) = query_shell_menu(paths, dir)?;
        let entries = unsafe { read_menu_level(Some(&cm), hmenu, 1, 0, load_icons) };
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyMenu(hmenu);
        }
        Ok(entries)
    })
}

#[cfg(windows)]
fn invoke_classic_verb_impl(
    paths: &[PathBuf],
    dir: &Path,
    offset: u32,
    expect_label: &str,
) -> anyhow::Result<()> {
    with_sta(|| {
        use windows::Win32::UI::Shell::CMINVOKECOMMANDINFO;
        let (cm, hmenu) = query_shell_menu(paths, dir)?;
        let result = unsafe {
            // Label guard: never fire a stale offset at the wrong command.
            let entries = read_menu_level(Some(&cm), hmenu, 1, 0, true);
            let mut found: Option<(u32, String)> = None;
            let mut stack = entries;
            while let Some(e) = stack.pop() {
                if !e.separator && e.offset == offset {
                    found = Some((e.offset, e.label.clone()));
                    break;
                }
                stack.extend(e.children);
            }
            match found {
                Some((off, label)) if label == expect_label => {
                    let hwnd = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
                    let info = CMINVOKECOMMANDINFO {
                        cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
                        hwnd,
                        lpVerb: windows::core::PCSTR(off as usize as *const u8),
                        nShow: 1, // SW_SHOWNORMAL
                        ..Default::default()
                    };
                    cm.InvokeCommand(&info)
                        .map_err(|e| anyhow::anyhow!("InvokeCommand: {e}"))
                }
                Some((_, label)) => anyhow::bail!(
                    "classic menu changed under us (want {expect_label:?}, found {label:?})"
                ),
                None => anyhow::bail!("classic verb offset {offset} no longer present"),
            }
        };
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyMenu(hmenu);
        }
        result
    })
}

/// Read one HMENU level into entries. `id_first` is the QueryContextMenu
/// base (1); offsets are `wID - id_first`. Recursion capped for safety.
#[cfg(windows)]
unsafe fn read_menu_level(
    cm: Option<&windows::Win32::UI::Shell::IContextMenu>,
    hmenu: windows::Win32::UI::WindowsAndMessaging::HMENU,
    id_first: u32,
    depth: u32,
    load_icons: bool,
) -> Vec<ClassicMenuEntry> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetMenuItemCount, GetMenuItemInfoW, MENUITEMINFOW, MFS_DISABLED, MFS_GRAYED,
        MFT_SEPARATOR, MIIM_BITMAP, MIIM_FTYPE, MIIM_ID, MIIM_STATE, MIIM_STRING, MIIM_SUBMENU,
    };

    let mut out = Vec::new();
    let count = GetMenuItemCount(Some(hmenu));
    if count <= 0 {
        return out;
    }
    for i in 0..count as u32 {
        let mut buf = vec![0u16; 1024];
        let mut mii: MENUITEMINFOW = std::mem::zeroed();
        mii.cbSize = std::mem::size_of::<MENUITEMINFOW>() as u32;
        mii.fMask = MIIM_STRING | MIIM_BITMAP | MIIM_SUBMENU | MIIM_STATE | MIIM_ID | MIIM_FTYPE;
        mii.dwTypeData = windows::core::PWSTR(buf.as_mut_ptr());
        mii.cch = buf.len() as u32;
        if GetMenuItemInfoW(hmenu, i, true, &mut mii).is_err() {
            continue;
        }
        if (mii.fType.0 & MFT_SEPARATOR.0) != 0 {
            out.push(ClassicMenuEntry {
                label: String::new(),
                offset: u32::MAX,
                enabled: false,
                separator: true,
                icon: None,
                children: Vec::new(),
            });
            continue;
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        // Strip '&' mnemonic markers (Files hides them too; "&&" → literal &).
        let label = String::from_utf16_lossy(&buf[..len])
            .replace("&&", "\u{1}")
            .replace('&', "")
            .replace('\u{1}', "&");
        if label.is_empty() && mii.hSubMenu.0.is_null() {
            continue;
        }

        // Initialize submenus so dynamic handlers (Send to, Open with, WinRAR, etc.) populate items and icons
        if !mii.hSubMenu.0.is_null() {
            if let Some(cm_ref) = cm {
                use windows::core::Interface;
                use windows::Win32::UI::Shell::{IContextMenu2, IContextMenu3};
                use windows::Win32::UI::WindowsAndMessaging::WM_INITMENUPOPUP;
                use windows::Win32::Foundation::{WPARAM, LPARAM, LRESULT};

                if let Ok(cm3) = cm_ref.cast::<IContextMenu3>() {
                    let mut lres = LRESULT(0);
                    let _ = cm3.HandleMenuMsg2(
                        WM_INITMENUPOPUP,
                        WPARAM(mii.hSubMenu.0 as usize),
                        LPARAM(i as isize),
                        Some(&mut lres),
                    );
                } else if let Ok(cm2) = cm_ref.cast::<IContextMenu2>() {
                    let _ = cm2.HandleMenuMsg(
                        WM_INITMENUPOPUP,
                        WPARAM(mii.hSubMenu.0 as usize),
                        LPARAM(i as isize),
                    );
                }
            }
        }

        let children = if !mii.hSubMenu.0.is_null() && depth < 4 {
            read_menu_level(cm, mii.hSubMenu, id_first, depth + 1, load_icons)
        } else {
            Vec::new()
        };
        let state = mii.fState.0;
        out.push(ClassicMenuEntry {
            label,
            offset: mii.wID.wrapping_sub(id_first),
            enabled: (state & MFS_DISABLED.0) == 0 && (state & MFS_GRAYED.0) == 0,
            separator: false,
            icon: if load_icons {
                hbitmap_to_render_image(mii.hbmpItem)
            } else {
                None
            },
            children,
        });
    }
    out
}

/// Convert a menu bitmap to a `RenderImage` (BGRA bytes straight from
/// `GetDIBits` — the same convention GPUI's raster-image path produces after
/// its R/B swap, so the shell bytes are used as-is). Returns None for
/// system/stock bitmap constants.
#[cfg(windows)]
fn hbitmap_to_render_image(
    hbmp: windows::Win32::Graphics::Gdi::HBITMAP,
) -> Option<Arc<gpui::RenderImage>> {
    use windows::Win32::Graphics::Gdi::{
        GetDIBits, GetDC, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS,
    };

    if (hbmp.0 as isize) <= 8 {
        // HBMMENU_SYSTEM / HBMMENU_CALLBACK / stock placeholders, not pixels.
        return None;
    }
    unsafe {
        let mut bm: BITMAP = std::mem::zeroed();
        if GetObjectW(
            windows::Win32::Graphics::Gdi::HGDIOBJ(hbmp.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut _ as *mut core::ffi::c_void),
        ) == 0
        {
            return None;
        }
        let (w, h) = (bm.bmWidth, bm.bmHeight);
        if w <= 0 || h <= 0 || w > 256 || h > 256 {
            return None;
        }
        let hdc = GetDC(None);
        if hdc.0.is_null() {
            return None;
        }
        let mut bih: BITMAPINFOHEADER = std::mem::zeroed();
        bih.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bih.biWidth = w;
        bih.biHeight = -h; // top-down: no row flip needed
        bih.biPlanes = 1;
        bih.biBitCount = 32;
        bih.biCompression = BI_RGB.0;
        let mut bi = BITMAPINFO {
            bmiHeader: bih,
            bmiColors: [std::mem::zeroed(); 1],
        };
        let stride = w as usize * 4;
        let mut buf = vec![0u8; stride * h as usize];
        let lines = GetDIBits(
            hdc,
            hbmp,
            0,
            h as u32,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            &mut bi as *mut _,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, hdc);
        if lines == 0 {
            return None;
        }
        // Shell bitmaps without alpha come back all-zero: treat as opaque.
        let has_alpha = buf.chunks_exact(4).any(|px| px[3] != 0);
        if !has_alpha {
            for px in buf.chunks_exact_mut(4) {
                px[3] = 255;
            }
        }
        let frame = image::Frame::new(
            image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(w as u32, h as u32, buf)?,
        );
        Some(Arc::new(gpui::RenderImage::new(smallvec::smallvec![
            frame
        ])))
    }
}
