# 03 — GPUI Frontend & Fluent Design Specification (`files-rs`)

**Target Modules:** `src/main.rs`, `src/app_render.rs`, `src/ui/constants.rs`, `src/ui/components/`, `src/ui/views/`  
**Framework:** Zed's `gpui = "0.2.2"` + `window-vibrancy = "0.7.1"`  
**Visual Target:** `Files-community` (Windows 11 WinUI 3 Fluent Design) at locked **120+ FPS / 144 Hz** (`< 6.94 ms` frame time)

---

## 1. Windows 11 Mica / Mica Alt & Translucent Surface Architecture

### 1.1 Window Creation & DWM Backdrop (`src/main.rs`)
To achieve true Windows 11 Mica / Mica Alt translucency without opaque black fills:
1. Read `appearance.mica_enabled` and `appearance.mica_alt` from `SettingsModel` before opening the window.
2. Set `WindowOptions::window_background` to:
   - `WindowBackgroundAppearance::MicaAltBackdrop` when `mica_enabled && mica_alt`
   - `WindowBackgroundAppearance::MicaBackdrop` when `mica_enabled && !mica_alt`
   - `WindowBackgroundAppearance::Opaque` when `!mica_enabled`
3. Inside `cx.open_window`, apply `window_vibrancy`:
```rust
#[cfg(windows)]
{
    if let Ok(raw_win) = raw_window_handle::HasWindowHandle::window_handle(window) {
        struct RawWinWrapper(raw_window_handle::RawWindowHandle);
        unsafe impl raw_window_handle::HasWindowHandle for RawWinWrapper {
            fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
                Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(self.0) })
            }
        }
        let wrapper = RawWinWrapper(raw_win.as_raw());
        if mica_enabled {
            let res = if mica_alt {
                window_vibrancy::apply_tabbed(&wrapper, Some(true))
            } else {
                window_vibrancy::apply_mica(&wrapper, Some(true))
            };
            if res.is_err() {
                let _ = window_vibrancy::apply_acrylic(&wrapper, Some((24, 24, 24, 200)));
            }
        }
    }
}
```

### 1.2 Semantic Fluent Color & Typography Tokens (`src/ui/constants.rs`)

| Token Constant | Value | WinUI 3 Equivalent | Purpose |
| :--- | :--- | :--- | :--- |
| `FONT_MAIN` / `FONT_FAMILY_DEFAULT` | `"Segoe UI Variable Text"` | `BodyStrongTextBlockStyle` / `BodyTextBlockStyle` | Primary UI & file list font family |
| `FONT_MDL2` | `"Segoe Fluent Icons"` | `SymbolThemeFontFamily` | Crisp Windows 11 vector icon glyphs |
| `BG_MAIN_RGBA` | `0x191919CC` (80% alpha) | `LayerOnMicaBaseAltFillColorDefault` | Content pane translucent tint |
| `BG_MAIN_MICA_ALT_RGBA` | `0x141414CC` (80% alpha) | `MicaAlt` base tint | Deeper content tint when `mica_alt` is active |
| `BG_SIDEBAR_RGBA` | `0x1F1F1FB3` (70% alpha) | `NavigationViewDefaultPaneBackground` | Translucent navigation rail |
| `BG_TAB_BAR_RGBA` | `0x14141499` (60% alpha) | `TabViewBackground` | Titlebar / tab strip over Mica Alt |
| `BG_STATUS_BAR_RGBA` | `0x1F1F1FB3` (70% alpha) | `StatusBarBackground` | Translucent bottom status bar |
| `CARD_STROKE_RGBA` | `0xFFFFFF0F` (5.9% alpha) | `CardStrokeColorDefault` | 1px subtle border stroke on cards & flyouts |
| `BG_ITEM_HOVER_RGBA` | `0xFFFFFF0F` (5.9% alpha) | `SubtleFillColorSecondary` | Row/tile hover highlight |
| `BG_ITEM_SELECTED_RGBA` | `0xFFFFFF18` (9.4% alpha) | `SubtleFillColorTertiary` | Row/tile selection fill (+ 3px accent pill) |

### 1.3 `AppBackground` Compositor (`src/ui/components/app_background.rs`)
`AppBackground::build(folder_bg, mica_enabled, mica_alt, children)` composites:
1. The base surface tint (`BG_MAIN_MICA_ALT_RGBA` or `BG_MAIN_RGBA` when `mica_enabled` is true, or solid `0x191919FF` when disabled).
2. An optional per-folder background image layer (`desktop.ini` `image_path`, `opacity`, and `BackgroundFit` mapped to `ObjectFit::Fill | Contain | Cover | ScaleDown`).
3. The foreground UI child stack.

---

## 2. Strict Viewport Virtualization (`uniform_list`)

### 2.1 Flat & Grouped Virtualization in `details_view.rs`
To maintain `< 6.94 ms` frame times on 50,000+ file directories:
- **Flat Details View (`details_view_flat`):** Uses `uniform_list("details-file-list", visible_count, ...)` with `track_scroll(&self.details_scroll_handle)`. Only rows inside `range: Range<usize>` are constructed per frame.
- **Grouped Details View (`details_view_grouped`):** Flattens collapsible group headers and visible items into `Rc<Vec<GroupedDetailsRow>>`:
```rust
#[derive(Clone)]
enum GroupedDetailsRow {
    Header {
        key: String,
        count: usize,
        is_collapsed: bool,
    },
    Item(usize),
}
```
And renders `rows.len()` via `uniform_list("details-grouped-list", total_rows, ...)`, ensuring grouped directories with 50,000+ items never fall back to unvirtualized `.overflow_y_scroll()` DOM trees.

### 2.2 Grid View & Columns View Virtualization
- **Grid View (`src/ui/views/grid_view.rs`):** Computes `cols = ((available_w - 16.0) / (card_w + gap)).floor().max(1.0) as usize` and virtualizes by **grid row** (`row_count = (visible.len() + cols - 1) / cols`) via `uniform_list`.
- **Columns View (`src/ui/views/columns_view.rs`):** Uses `uniform_list("columns-file-list", visible_len, ...)` for the active directory column (`col2_scroll_handle`).

---

## 3. Single-Clock Display-Refresh-Rate Physics & WinUI 3 Easing (`src/ui/components/bouncy_scroll.rs`)

### 3.1 Single-Clock Momentum & Spring-Back Loop
- `bouncy_scroll_handler_with_handle` accumulates scroll wheel velocity (`VELOCITY_BLEND_HISTORY = 0.35`), applies rubber-band resistance (`RUBBER_BAND_RESISTANCE = 0.35`, `MAX_OVERSCROLL = 60.0`), updates the `UniformListScrollHandle` offset immediately, and sets `state.animating = true`.
- `AppWindow::render()` calls `self.tick_bouncy_scrolls(_window, cx)` once per frame using the measured frame delta `dt` (`last_render_time`) and requests the next VSync frame via `_window.request_animation_frame()` while `any_active` is true.
- No detached `8ms` background timers are spawned during scroll events, eliminating dual-clock phase beating on 120 Hz / 144 Hz / 240 Hz displays.

### 3.2 WinUI 3 Cubic-Bezier Easing Functions
`src/ui/components/bouncy_scroll.rs` provides Newton-Raphson cubic-bezier evaluators matching Windows 11 WinUI 3 motion tokens:
- `fluent_entrance_ease(t: f32) -> f32`: `cubic-bezier(0.1, 0.9, 0.2, 1.0)` (Direct Enter — 167ms–250ms flyouts, dialogs, selection pills)
- `fluent_exit_ease(t: f32) -> f32`: `cubic-bezier(0.7, 0.0, 1.0, 0.5)` (Fast Exit — 120ms dismissal)
