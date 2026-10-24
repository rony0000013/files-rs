//! WYSIWYG Office preview by hosting the OS-registered `IPreviewHandler`
//! (the same renderer Explorer uses) in a child HWND over the preview pane.
//!
//! Works only when a preview handler is registered for the extension
//! (this machine: Word/PowerPoint/Excel previewers ship with Office).
//! Anything else — lookup failure, COM failure, missing handler — returns
//! `false` and callers keep the Markdown fallback.
//!
//! Threading: everything runs on the GPUI UI thread (STA). The child HWND is
//! repositioned idempotently from the render path (`sync`), so scrolling and
//! splitter drags track without extra timers.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use windows::core::GUID;

/// Which pane region the host element is painting into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostSlot {
    Side,
    Peek,
}

/// Window-relative logical bounds captured by the host element each frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct HostBounds {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub scale: f32,
}

struct Inner {
    child: Option<windows::Win32::Foundation::HWND>,
    handler: Option<windows::Win32::UI::Shell::IPreviewHandler>,
    path: Option<PathBuf>,
    parent: Option<windows::Win32::Foundation::HWND>,
    last_rect: Option<(i32, i32, i32, i32)>,
    visible: bool,
}

#[derive(Clone)]
pub struct OfficePreviewHost {
    inner: Arc<Mutex<Inner>>,
    pub side: Arc<Mutex<Option<HostBounds>>>,
    pub peek: Arc<Mutex<Option<HostBounds>>>,
    com_init: Arc<std::sync::OnceLock<bool>>,
}

impl std::fmt::Debug for OfficePreviewHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let guard = self.inner.lock();
        f.debug_struct("OfficePreviewHost")
            .field(
                "path",
                &guard.as_ref().ok().and_then(|i| i.path.clone()),
            )
            .field("visible", &guard.as_ref().map(|i| i.visible).unwrap_or(false))
            .finish()
    }
}

impl Default for OfficePreviewHost {
    fn default() -> Self {
        Self::new()
    }
}

impl OfficePreviewHost {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                child: None,
                handler: None,
                path: None,
                parent: None,
                last_rect: None,
                visible: false,
            })),
            side: Arc::new(Mutex::new(None)),
            peek: Arc::new(Mutex::new(None)),
            com_init: Arc::new(std::sync::OnceLock::new()),
        }
    }

    /// STA for COM on the calling (UI) thread. `false` = keep Markdown fallback.
    #[cfg(windows)]
    fn ensure_sta(&self) -> bool {
        *self.com_init.get_or_init(|| unsafe {
            use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok()
        })
    }

    /// Resolve the preview-handler CLSID for an extension via HKCR shellex keys.
    #[cfg(windows)]
    pub fn handler_clsid_for_ext(ext: &str) -> Option<GUID> {
        let ext = ext.trim_start_matches('.').to_ascii_lowercase();
        let hkcr = winreg::RegKey::predef(winreg::enums::HKEY_CLASSES_ROOT);
        // This machine registers under ...250836f; other setups use ...af7f68d.
        for key in [
            "8895b1c6-b41f-4c1c-a562-0d564250836f",
            "8895b1c6-b41f-4c1c-a562-0d564af7f68d",
        ] {
            let path = format!(".{}\\shellex\\{{{}}}", ext, key);
            if let Ok(k) = hkcr.open_subkey(&path) {
                if let Ok(clsid) = k.get_value::<String, _>("") {
                    if let Some(g) = parse_guid(&clsid) {
                        return Some(g);
                    }
                }
            }
        }
        None
    }

    /// Find the app's top-level GPUI window ("Zed::Window" class, own PID).
    #[cfg(windows)]
    fn main_hwnd() -> Option<windows::Win32::Foundation::HWND> {
        use windows::Win32::Foundation::{HWND, LPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GetClassNameW, IsWindowVisible,
        };
        let pid = std::process::id();
        let found = std::sync::Arc::new(std::sync::Mutex::new(None::<HWND>));
        let found_cb = found.clone();
        unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
            let ctx = &*(lparam.0 as *const (u32, std::sync::Arc<std::sync::Mutex<Option<HWND>>>));
            let mut got = 0u32;
            windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, Some(&mut got));
            if got != ctx.0 || IsWindowVisible(hwnd).as_bool() == false {
                return true.into();
            }
            let mut cls = [0u16; 64];
            let n = GetClassNameW(hwnd, &mut cls);
            if n > 0 {
                let name = String::from_utf16_lossy(&cls[..n as usize]);
                if name == "Zed::Window" {
                    if let Ok(mut f) = ctx.1.lock() {
                        if f.is_none() {
                            *f = Some(hwnd);
                        }
                    }
                }
            }
            true.into()
        }
        let ctx = Box::new((pid, found_cb));
        let raw = Box::into_raw(ctx);
        let _ = unsafe { EnumWindows(Some(enum_cb), LPARAM(raw as isize)) };
        let _ctx = unsafe { Box::from_raw(raw) };
        drop(_ctx);
        found.lock().ok()?.clone()
    }

    /// Show the OS preview for `path` (same file: just ensure visible).
    /// Returns false when anything is missing -> caller keeps Markdown.
    #[cfg(not(windows))]
    pub fn show_for(&self, _path: &Path, _ext: &str) -> bool {
        false
    }

    /// Show the OS preview for `path` (same file: just ensure visible).
    /// Returns false when anything is missing -> caller keeps Markdown.
    #[cfg(windows)]
    pub fn show_for(&self, path: &Path, ext: &str) -> bool {
        if !self.ensure_sta() {
            return false;
        }
        let mut inner = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return false,
        };
        if inner.path.as_deref() == Some(path) && inner.handler.is_some() {
            inner.visible = true;
            return true;
        }
        // Tear down previous.
        Self::destroy_locked(&mut inner);
        let clsid = match Self::handler_clsid_for_ext(ext) {
            Some(g) => g,
            None => return false,
        };
        let parent = match Self::main_hwnd() {
            Some(h) => h,
            None => return false,
        };
        let ok = unsafe { self.create_locked(&mut inner, &clsid, path, parent) };
        if ok {
            inner.visible = true;
        }
        ok
    }

    #[cfg(windows)]
    unsafe fn create_locked(
        &self,
        inner: &mut Inner,
        clsid: &GUID,
        path: &Path,
        parent: windows::Win32::Foundation::HWND,
    ) -> bool {
        use windows::Win32::Foundation::{HINSTANCE, HWND, RECT};
        use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
        use windows::core::Interface;
        use windows::Win32::UI::Shell::IPreviewHandler;
        use windows::Win32::UI::Shell::PropertiesSystem::IInitializeWithFile;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, WINDOW_EX_STYLE, WINDOW_STYLE, WS_CHILD,
        };
        use windows::core::{w, HSTRING};

        let handler: IPreviewHandler =
            match CoCreateInstance(clsid, None, CLSCTX_INPROC_SERVER) {
                Ok(h) => h,
                Err(_) => return false,
            };
        let init: IInitializeWithFile = match handler.cast() {
            Ok(i) => i,
            Err(_) => return false,
        };
        let hpath = HSTRING::from(path.as_os_str());
        // STGM_READ = 0
        if init.Initialize(&hpath, 0).is_err() {
            return false;
        }
        let child = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("files-rs-office-preview"),
            WINDOW_STYLE(WS_CHILD.0),
            0,
            0,
            16,
            16,
            Some(parent),
            None,
            None,
            None,
        ) {
            Ok(h) => h,
            Err(_) => return false,
        };
        let rect = RECT { left: 0, top: 0, right: 16, bottom: 16 };
        if handler.SetWindow(child, &rect).is_err() {
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(child);
            return false;
        }
        if handler.DoPreview().is_err() {
            let _ = handler.Unload();
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(child);
            return false;
        }
        let _ = HINSTANCE;
        let _ = HWND;
        inner.child = Some(child);
        inner.handler = Some(handler);
        inner.path = Some(path.to_path_buf());
        inner.parent = Some(parent);
        inner.last_rect = None;
        true
    }

    #[cfg(windows)]
    fn destroy_locked(inner: &mut Inner) {
        use windows::Win32::UI::WindowsAndMessaging::DestroyWindow;
        if let Some(h) = inner.handler.take() {
            unsafe {
                let _ = h.Unload();
            }
        }
        if let Some(c) = inner.child.take() {
            unsafe {
                let _ = DestroyWindow(c);
            }
        }
        inner.path = None;
        inner.last_rect = None;
        inner.visible = false;
    }

    /// Hide (and unload) the hosted preview, e.g. on selection change.
    #[cfg(not(windows))]
    pub fn hide(&self) {}

    /// Hide (and unload) the hosted preview, e.g. on selection change.
    #[cfg(windows)]
    pub fn hide(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            Self::destroy_locked(&mut inner);
        }
        if let Ok(mut s) = self.side.lock() {
            *s = None;
        }
        if let Ok(mut p) = self.peek.lock() {
            *p = None;
        }
    }

    #[cfg(not(windows))]
    pub fn sync(&self, _want_visible: bool, _peek_open: bool, _scroll_y_px: f32) {}

    /// Idempotent per-frame sync from the render path: pick the active slot
    /// (peek wins while the modal is open), position the child HWND over the
    /// scrolled content rect, show/hide as needed.
    #[cfg(windows)]
    pub fn sync(&self, want_visible: bool, peek_open: bool, scroll_y_px: f32) {
        let slot = if peek_open { &self.peek } else { &self.side };
        let bounds = slot.lock().ok().and_then(|s| *s);
        let mut inner = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if inner.child.is_none() || inner.handler.is_none() {
            return;
        }
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, ShowWindow, SW_HIDE, SW_SHOWNOACTIVATE, HWND_TOP, SET_WINDOW_POS_FLAGS, SWP_NOACTIVATE, SWP_NOZORDER};
        let show = want_visible && bounds.is_some();
        if show != inner.visible {
            unsafe {
                let _ = ShowWindow(
                    inner.child.unwrap(),
                    if show { SW_SHOWNOACTIVATE } else { SW_HIDE },
                );
            }
            inner.visible = show;
        }
        if !show {
            return;
        }
        let b = bounds.unwrap();
        // Content scrolls under a fixed pane: offset the recorded origin.
        let x = (b.x * b.scale).round() as i32;
        let y = ((b.y - scroll_y_px) * b.scale).round() as i32;
        let w = (b.w * b.scale).round().max(16.0) as i32;
        let h = (b.h * b.scale).round().max(16.0) as i32;
        let rect = (x, y, x + w, y + h);
        if inner.last_rect != Some(rect) {
            unsafe {
                let _ = SetWindowPos(
                    inner.child.unwrap(),
                    Some(HWND_TOP),
                    x,
                    y,
                    w,
                    h,
                    SET_WINDOW_POS_FLAGS(SWP_NOACTIVATE.0 | SWP_NOZORDER.0),
                );
            }
            inner.last_rect = Some(rect);
            if let Some(handler) = inner.handler.as_ref() {
                let r = RECT { left: 0, top: 0, right: w, bottom: h };
                unsafe {
                    let _ = handler.SetRect(&r);
                }
            }
        }
    }
}

fn parse_guid(s: &str) -> Option<GUID> {
    let hex: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 32 {
        return None;
    }
    u128::from_str_radix(&hex, 16).ok().map(GUID::from_u128)
}

/// GPUI leaf element that records its painted bounds so the native child
/// HWND can be positioned over the preview content region.
pub struct OfficeHostElement {
    slot: Arc<Mutex<Option<HostBounds>>>,
    min_h: f32,
}

impl OfficeHostElement {
    pub fn new(slot: Arc<Mutex<Option<HostBounds>>>) -> Self {
        Self { slot, min_h: 420.0 }
    }
}

impl gpui::Element for OfficeHostElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        let mut style = gpui::Style::default();
        style.flex_grow = 1.0;
        style.min_size.height = gpui::px(self.min_h).into();
        (window.request_layout(style, std::iter::empty(), cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        _bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut gpui::Window,
        _cx: &mut gpui::App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut gpui::Window,
        _cx: &mut gpui::App,
    ) {
        if let Ok(mut slot) = self.slot.lock() {
            *slot = Some(HostBounds {
                x: bounds.origin.x.into(),
                y: bounds.origin.y.into(),
                w: bounds.size.width.into(),
                h: bounds.size.height.into(),
                scale: window.scale_factor(),
            });
        }
    }
}

impl gpui::IntoElement for OfficeHostElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}
