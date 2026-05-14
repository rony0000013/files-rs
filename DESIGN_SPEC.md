# Files-RS — Design & Backend Specification

> This document defines the exact visual design targets and backend logic required for the Rust/GPUI port to match Files-community. Read alongside TODO.md.
> **Rule**: Every measurement, color, glyph, and behavior here comes directly from the Files-community source (XAML, CS, constants). Do not deviate without documenting why.

---

## 1. Window & Shell Chrome

### 1.1 Mica Background Material
- **API**: Call `window-vibrancy::apply_mica(hwnd, Some(true))` immediately after window creation, before first paint. `Some(true)` = dark tint variant on dark theme; `Some(false)` = light.
- **Fallback on Windows 10**: Mica is unavailable. Fall back to `apply_acrylic(hwnd, Some(0x01000000))` (near-opaque Acrylic). On failure, use a solid `#202020` (dark) / `#F3F3F3` (light) background.
- **Re-apply on theme change**: Listen for `WM_SETTINGCHANGE` with `lParam = "ImmersiveColorSet"`. Re-call apply_mica with updated dark/light flag.
- **DWM extend frame**: Call `DwmExtendFrameIntoClientArea` with `MARGINS { -1, -1, -1, -1 }` to extend the Mica effect over the entire client area. This is what makes the window background material visible.
- **Transparency of all panels**: The NavBar, Sidebar, and content area backgrounds must be **semi-transparent or fully transparent** so the Mica material shows through. Never use opaque fills for primary surfaces.
- **Mica Alt Support**: Support the "Mica Alt" backdrop material (`BackdropMaterialType.MicaAlt`) for a more saturated, high-contrast variant of standard Mica.

### 1.2 Titlebar & Caption Area
- **No native titlebar**: Set `titlebar: None` in GPUI WindowOptions (or set `appears_transparent: true` with an empty title). The entire top strip is custom-rendered.
- **Caption buttons (Min/Max/Close)**: These are native Win32 HTCAPTION-area controls. They are drawn by DWM. Ensure the top-right 138px (3×46px) of the TabBar row is excluded from the custom hit-test region so DWM handles them natively.
- **Drag region**: Register the remaining TabBar area (minus tabs themselves and caption buttons) as `HTCAPTION` via `WM_NCHITTEST`. In GPUI this maps to the transparent `DragAreaRectangle` that fills the right side of the tab strip footer.
- **Window title for taskbar**: Even without a visible titlebar, set the Win32 window title to the current tab's folder name via `SetWindowTextW`. Update on every tab/navigation change.
- **Multi-Process Architecture**: The application relies on a secondary `Files-RS-Launcher.exe` to handle OS-level redirection and registry-based "Set as Default File Manager" hooks.
- **System Dialog Replacement**: The app can optionally replace the system Open/Save dialogs by injecting/registering specialized DLLs (`Files.App.OpenDialog64.dll`) via `regsvr32`.

### 1.3 Window Sizing & Responsive Breakpoints
- **Default size on first launch**: 1280×800 px, centered on primary monitor.
- **Minimum size**: 640×480 px enforced via `WM_GETMINMAXINFO`.
- **Sidebar hidden breakpoint**: When window width < 750px (`MultiplePaneWidthThreshold`), the dual-pane button becomes unavailable. The sidebar can still be toggled manually.
- **Navbar collapse breakpoint**: When window width < 540px, the Forward, Up, and Refresh buttons collapse into a single `...` overflow button (`FlyoutButton`). The Back button always remains visible.
- **Sidebar resize**: User can drag the splitter between sidebar and content. Clamp: min 180px, max 500px. Persist width in config.
- **Compact Overlay (PiP)**: Support a mini-window mode (`ToggleCompactOverlayAction`) that keeps a simplified File Explorer view always-on-top.
- **System Tray Persistence**: Support "Leave App Running" (`GeneralSettingsService.LeaveAppRunning`) which hides the window to the system tray instead of closing it, enabling instant re-launch.

### 1.4 Window Layout Stack (pixel-precise)
```
┌──────────────────────────────────────────────────────────────┐  ← y=0
│  [TabBar — 40px total height, Mica background, HTCAPTION]   │
│  ├─ Tab strip: tabs sit at y=10, height=30, radius-top=8px  │
│  └─ [⊞ 30×30][Tab1][Tab2][Tab3][+30×30]  ··· [─][□][✕]     │
├──────────────────────────────────────────────────────────────┤  ← y=40
│  [NavigationToolbar — 48px, transparent bg, px 4,0,4,0]     │
│  [☰36×32][←36×32][→36×32][↑36×32][↻36×32] [Omnibar-flex]  │
│  [ShelfToggle 36×32][StatusCenter 36×32][UpdateBtn? 36×32]  │
├─────────────────┬────────────────────────────────────────────┤  ← y=88
│ Sidebar         │  [Toolbar — 40px, shown if enabled]        │
│ 250px default   ├────────────────────────────────────────────┤  ← y=128
│ min:180 max:500 │  [File Content Area — flex, overflow:auto] │
│                 │                                            │
│ [▌] ← splitter │  (Details / Grid / Columns / Home views)   │
│                 │                                            │
│                 ├────────────────────────────────────────────┤
│                 │  [StatusBar — 32px, padding 8px left]      │
└─────────────────┴────────────────────────────────────────────┘
```

### 1.5 Multi-Pane Layout (Dual Pane)
When dual pane is active, the File Content Area splits:
```
┌───────────────────────┬──┬────────────────────────┐
│  Primary Pane         │▌ │  Secondary Pane         │
│  (active: accent rim) │  │  (inactive: dim 70%)    │
└───────────────────────┴──┴────────────────────────┘
```
- Splitter: 4px wide, `GridSplitter` style (`DefaultGridSplitterStyle.xaml`), hover = accent color.
- Pane focus: clicking inside a pane makes it active. Active pane has a 2px accent-colored top border.
- Panes can be arranged horizontally (side-by-side, default) or vertically (stacked).
- Each pane has its own independent NavBar, file list, and selection state.
- Tab strip applies to the whole window, not per-pane.

---

## 2. Color System & Theme Tokens

### 2.1 WinUI Theme Token Mapping

Files-community surfaces are built on WinUI 3 semantic color tokens that adapt to system theme (Light/Dark) and accent color. Every GPUI color must map to an equivalent semantic meaning.

| WinUI Token | Dark Mode RGBA | Light Mode RGBA | Usage |
|---|---|---|---|
| `App.Theme.AddressBar.BackgroundBrush` | `#00000000` | `#00000000` | NavBar background (Mica shows through) |
| `SubtleFillColorSecondary` | `#0FFFFFFF` (6.3%) | `#06000000` (3.7%) | Item hover background |
| `SubtleFillColorTertiary` | `#18FFFFFF` (9.4%) | `#0A000000` (5.9%) | Item selected/pressed background |
| `SubtleFillColorTransparent` | `#00000000` | `#00000000` | Default (no-fill) state |
| `LayerFillColorDefault` | `#4D000000` (30%) | `#FFFFFFFF` (100%) | Card / flyout surface fill |
| `CardBackgroundFillColorDefault` | `#0DFFFFFF` (5.1%) | `#B3FFFFFF` (70%) | In-content card bg |
| `AcrylicBackgroundFillColorDefault` | Acrylic over Mica | Acrylic over Mica | Flyout and pane surfaces |
| `TextFillColorPrimary` | `#E4FFFFFF` (89%) | `#E4000000` (89%) | Primary text (file names, labels) |
| `TextFillColorSecondary` | `#9DFFFFFF` (62%) | `#9D000000` (62%) | Secondary text (sizes, dates) |
| `TextFillColorDisabled` | `#5CFFFFFF` (36%) | `#5C000000` (36%) | Disabled state labels |
| `DividerStrokeColorDefault` | `#15FFFFFF` (8.3%) | `#14000000` (7.8%) | 1px dividers between sections |
| `ControlStrokeColorDefault` | `#12FFFFFF` (7%) | `#0F000000` (6%) | Button/input outlines |
| `AccentFillColorDefault` | System accent `RGB` at 100% | Same | Active tab stripe, selection accent |
| `AccentFillColorSecondary` | Accent at 90% | Accent at 90% | Hover on accented elements |
| `SystemFillColorSuccess` | `#6CCB5F` | `#0F7B0F` | Completed operation badge |
| `SystemFillColorCritical` | `#FF99A4` | `#C42B1C` | Failed operation badge |
| `SystemFillColorAttention` | `#FCE100` | `#9D5D00` | In-progress operation ring |

### 2.2 Accent Color: Runtime Detection
```rust
fn read_accent_color() -> Rgba {
    // HKEY_CURRENT_USER\Software\Microsoft\Windows\DWM
    // Value: "AccentColor" (DWORD) — stored as ABGR not ARGB!
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let dwm = hkcu.open_subkey("Software\\Microsoft\\Windows\\DWM").ok();
    if let Some(key) = dwm {
        let abgr: u32 = key.get_value("AccentColor").unwrap_or(0xFF0078D4);
        // Convert ABGR → RGBA for GPUI
        let a = ((abgr >> 24) & 0xFF) as u8;
        let b = ((abgr >> 16) & 0xFF) as u8;
        let g = ((abgr >>  8) & 0xFF) as u8;
        let r = (abgr        & 0xFF) as u8;
        return Rgba { r: r as f32/255., g: g as f32/255., b: b as f32/255., a: a as f32/255. };
    }
    // Windows 11 blue default: #0078D4
    Rgba { r: 0., g: 0.471, b: 0.831, a: 1. }
}
// Re-read on WM_SETTINGCHANGE("ImmersiveColorSet") — accent can change while app is running.
```

### 2.3 Item State Colors (used everywhere: list rows, sidebar items, grid cells)
All interactive item backgrounds follow this state machine:
```
Default   → #00000000  (transparent)
Hover     → SubtleFillColorSecondary  (#0FFFFFFF dark / #06000000 light)
Pressed   → SubtleFillColorTertiary   (#18FFFFFF dark / #0A000000 light)
Selected  → SubtleFillColorTertiary + 2px left accent border
Selected+Hover → slightly brighter than Selected
Disabled  → opacity 0.4 on everything inside the item (DimItemOpacity = 0.4f)
```

### 2.4 Dark / Light Theme Detection
```rust
fn is_dark_theme() -> bool {
    // HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize
    // AppsUseLightTheme: 0 = dark, 1 = light
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
    if let Ok(key) = hkcu.open_subkey(path) {
        let light: u32 = key.get_value("AppsUseLightTheme").unwrap_or(0);
        return light == 0;
    }
    true // default dark
}
```
- **Re-check on `WM_SETTINGCHANGE("ImmersiveColorSet")` and update all GPUI theme tokens.
- **App Background Image**: Users can specify a global background image (`AppThemeBackgroundImageSource`) with configurable `AppThemeBackgroundImageOpacity`, `Stretch` (Fit/Tile/Center), and alignment settings. This image is rendered behind all panels using Mica/Acrylic as a semi-transparent filter.
- **Inactive Pane Dimming**: In dual-pane mode, the inactive pane's background color shifts to `AppThemeFileAreaSecondaryBackgroundColor` to emphasize the current focus.

### 2.5 High-Contrast Mode
When `SystemParametersInfo(SPI_GETHIGHCONTRAST)` returns `HCF_HIGHCONTRASTON`:
- Disable all transparency effects (Mica, Acrylic).
- Use solid `#000000` background, `#FFFFFF` text (or system high-contrast colors).
- Remove rounded corners (set radius to 0).
- Remove drop shadows.
- Replace icon-only buttons with text labels.

---

## 3. Typography

### 3.1 Font Family Detection
```rust
fn system_font() -> &'static str {
    // Segoe UI Variable ships with Windows 11 (build 22000+)
    use windows::Win32::System::SystemInformation::GetVersionExW;
    // OSVERSIONINFOW.dwBuildNumber >= 22000
    if windows_build_number() >= 22000 { "Segoe UI Variable" } else { "Segoe UI" }
}
```
Never hard-code the font string. GPUI must load this font at startup and use it for all text rendering.

### 3.2 Font Scale Table

| Context | Family | Size (px) | Weight | Line Height |
|---|---|---|---|---|
| File/folder names in list | Segoe UI Variable Text | 14 | Regular (400) | 20px |
| File names in grid (2-line) | Segoe UI Variable Text | 12 | Regular (400) | 16px |
| Column headers | Segoe UI Variable Text | 12 | SemiBold (600) | 16px |
| Timestamps, sizes, types | Segoe UI Variable Text | 12 | Regular (400) | 16px |
| Sidebar section headers | Segoe UI Variable Small | 11 | SemiBold (600) | 14px |
| Sidebar item labels | Segoe UI Variable Text | 13 | Regular (400) | 18px |
| Tab labels | Segoe UI Variable Text | 12 | Regular (400) | 16px |
| Status bar text | Segoe UI Variable Text | 12 | Regular (400) | 16px |
| NavBar breadcrumb segments | Segoe UI Variable Text | 14 | Regular (400) | 20px |
| NavBar path edit input | Segoe UI Variable Text | 14 | Regular (400) | 20px |
| Toolbar button labels | Segoe UI Variable Text | 12 | Regular (400) | 16px |
| Dialog titles | Segoe UI Variable Display | 20 | SemiBold (600) | 28px |
| Dialog body | Segoe UI Variable Text | 14 | Regular (400) | 20px |
| Group headers in file list | Segoe UI Variable Text | 13 | SemiBold (600) | 18px |
| Empty folder indicator | Segoe UI Variable Text | 14 | Regular (400) | 20px |

### 3.3 Text Rendering Rules
- **Ellipsis**: All single-line labels truncate with `…` when they overflow their container. Never wrap file names in single-line contexts.
- **Two-line grid names**: Grid view names may wrap to a maximum of 2 lines. After line 2, truncate with `…`.
- **No bold for regular content**: Do not bold file names or dates — this matches the Files-community aesthetic of clean, regular-weight body text.
- **Selection highlight**: When text is selected (rename mode), use system selection colors, not custom ones.

Detect font at runtime: if `Windows 10 Build >= 22000` → "Segoe UI Variable" else "Segoe UI".

### 3.4 Relative Date Formatting
All timestamps support a "Relative" display mode (`ApplicationDateTimeFormatter`):
- **< 1 min**: "Just now"
- **< 1 hour**: "X minutes ago"
- **< 24 hours**: "X hours ago"
- **< 7 days**: "X days ago"
- **> 7 days**: Standard date format ("D")
The UI must periodically refresh these labels every 60 seconds.

---

## 4. TabBar

The TabBar occupies the titlebar region of the window. It is simultaneously the tab manager and the OS window drag handle. In Files-community this is `TabBar.xaml` + `TabBarStyles.xaml`.

### 4.1 Dimensions & Spacing
- **Strip height**: 40px total. Tabs have `margin-top: 10px`, making each tab 30px tall.
- **Tab min width**: 80px (compressed when many tabs are open).
- **Tab max width**: 240px (at this point tabs stop growing even with fewer tabs).
- **Tab horizontal padding**: 8px left, 4px right (before close button).
- **Corner radius** (top corners only): 8px on active tab, 6px on inactive tabs.
- **Tab strip left margin**: 4px from the left edge (space for the "tab actions" button).
- **Tab strip right margin**: right-pad enough for the native Win32 caption buttons (approx 138px).
- **Add-tab button (+)**: 30×30, `padding: 8px`, glyph `E710` at 12px, positioned immediately right of last tab.
- **Tab actions button**: 30×30, `margin: 4px 0 -2px 4px`, glyph is single-pane icon `App.ThemedIcons.Panes.Single`. Left of tab strip.

### 4.2 Tab Appearance by State
```
Inactive tab:
  background:      transparent
  on hover:        SubtleFillColorSecondary (#0FFFFFFF)
  corner radius:   top-left 6px, top-right 6px
  close btn:       hidden until hover

Active tab:
  background:      LayerFillColorDefault (#0DFFFFFF dark / white light)
  border-bottom:   none (tab blends into content area)
  corner radius:   top-left 8px, top-right 8px
  close btn:       always visible
  text color:      TextFillColorPrimary (full opacity)

Inactive text color:  TextFillColorSecondary (62% opacity)
```

### 4.3 Tab Icon
- 16×16 shell icon for the current folder, placed 8px left of the tab label.
- Source: `SHGetFileInfoW` with `SHGFI_ICON | SHGFI_SMALLICON | SHGFI_OVERLAYINDEX`.
- Special folder icons: use `imageres.dll` index constants (Desktop=183, Downloads=184, etc.).
- Fallback: generic folder glyph `E8B7` from Segoe MDL2 Assets at 14px.

### 4.4 Close Button on Tab
- Size: 16×16, positioned 4px from right edge of tab.
- Glyph: `E8BB` (ChromeClose) from Segoe Fluent Icons, font size 10px.
- Background on hover: `SubtleFillColorSecondary`, corner radius 4px.
- Background on press: `SubtleFillColorTertiary`.
- Tooltip: "Close tab" (no hotkey shown here — hotkey is Ctrl+W and shown in tab context menu).

### 4.5 Tab Context Menu
Right-clicking any tab opens a `MenuFlyout` with these items in order:
1. New Tab (Ctrl+T)
2. Duplicate Tab
3. Move Tab to New Window
4. --- separator ---
5. Close Tabs to the Left
6. Close Tabs to the Right
7. Close Other Tabs
8. Reopen Closed Tab (Ctrl+Shift+T)

### 4.6 Drag & Drop
- **Reorder tabs**: Drag a tab left or right. `CanReorderTabs = true`, `CanDragTabs = true`.
- **Tear out to new window**: If dragged outside the tab strip area (`TabDroppedOutside`), open a new `AppWindow` with that tab's state.
- **Drop files onto tab**: Dropping files on an inactive tab switches to it and then completes the drop.
- **Scroll tabs**: Mouse wheel on the tab strip scrolls through tabs when overflow occurs.

### 4.7 Tab Actions Button Flyout (left of tab strip)
Opens `MenuFlyout` with:
- New Window
- Enter Compact Overlay (only if not already in compact overlay)
- Exit Compact Overlay (only if in compact overlay)
- Split Pane submenu → Vertically / Horizontally
- Arrange Panes submenu → Vertical ✓ / Horizontal ✓ (checkmarks show current)
- Close Active Pane (only if dual-pane active)

### 4.8 Drag Area (window drag handle)
A transparent `Rectangle` fills all space in the tab strip footer to the right of the `+` button. This region is registered as `HTCAPTION` so the window can be dragged from there. Files-community calls this `DragAreaRectangle`.

### 4.9 State Persistence
```rust
#[derive(Serialize, Deserialize, Clone)]
struct TabState {
    path: FilesPath,                      // current directory
    view_mode: ViewMode,                  // Details, Grid, Columns
    scroll_offset_px: f32,               // restore scroll on re-open
    sort: SortState,                      // per-tab sort prefs
    group_by: Option<GroupColumn>,
    history_back: VecDeque<FilesPath>,    // max 50
    history_forward: VecDeque<FilesPath>, // max 50
    secondary_pane_path: Option<FilesPath>, // if dual-pane was active
}

// On app close: serialize Vec<TabState> → JSON → write to config dir
// On app launch: deserialize and open tabs in order, focusing last active index
// On tab close: push to `closed_tabs: VecDeque<TabState>` (max 10) for reopen
// Crashed Session Recovery: App maintains a `running` flag in local state. If app starts and flag is still true, prompt `RestoreCrashedSessionDialog` using `LastCrashedTabList`.
```

---

## 5. Navigation Bar (Omnibar)

The NavBar is `NavigationToolbar.xaml`. Total height: **48px**, background: `App.Theme.AddressBar.BackgroundBrush` (transparent). The entire bar sits on top of the Mica material.

### 5.1 Left Button Strip

All buttons share the `AddressToolbarButtonStyle`:
- Size: **36px wide × 32px tall**
- Padding: 0
- Background: transparent / transparent border
- Foreground: `App.Theme.IconBaseBrush`
- Hover: `ButtonBackgroundPointerOver` (system theme)
- Disabled: 40% opacity, no interaction

| Button | Glyph | Hotkey | Tooltip | Notes |
|---|---|---|---|---|
| Sidebar toggle | `E700` (Hamburger) | none | "Toggle Sidebar" | `ToggleButton` (checked = open) |
| Back | `E72B` | Alt+Left | "Navigate Back (Alt+Left)" | Right-click = back history flyout |
| Forward | `E72A` | Alt+Right | "Navigate Forward (Alt+Right)" | Right-click = forward history flyout |
| Up | `E74A` | Alt+Up | "Navigate Up (Alt+Up)" | Right-click = parent history |
| Refresh | `E72C` | F5 / Ctrl+R | "Refresh (F5)" | always visible |
| Overflow `...` | `E712` | none | "See More" | Visible only when width < 540px. |

**History flyouts** (back & forward buttons, right-click):
- `MenuFlyout` with `MaxHeight: 400px`, `ScrollViewer.VerticalScrollMode: Auto`.
- Each item = a `MenuFlyoutItem` with folder name + icon.
- Clicking an item navigates directly to that history entry (skipping intermediate entries).
- Placement: `BottomEdgeAlignedLeft`.

### 5.4 Navigation Context & Special Interactions
- **Chevron button on each segment**: `▾` opens `MenuFlyout` listing sibling directories in that parent folder, enabling quick lateral navigation.
- **Double-click Empty Space**: Double-clicking the empty background area of the breadcrumb bar or the file list navigates to the parent directory (`DoubleClickToGoUp`).
- **Navigation Context**: When navigating "Up", the view must scroll to and highlight the folder just exited (`ScrollToPreviousFolderWhenNavigatingUp`).

### 5.2 Omnibar (center, flex-grow, column spacing 4px)

The Omnibar is a mode-switching control. Three named modes tab across the same space:

#### Mode A: Path / Breadcrumb (default, `IsDefault = true`)
**Inactive state** — rendered as `BreadcrumbBar`:
- Root item: 16×16 `Home.png` image, tooltip "Home".
- Segments: one `BreadcrumbBarItem` per path component.
  - Label text: folder name (not full path).
  - Each item: `AllowDrop = true` (drag files onto it to move there).
  - Chevron button on each item: `▾` opens `MenuFlyout` listing sibling directories in that parent folder, enabling quick lateral navigation without typing.
  - Ellipsis button: appears when path is too long to show all segments; collapses oldest segments. Tooltip: "Path is too long, click to see full path".
  - Clicking a segment: navigate to that path.
  - Clicking the bar background (non-segment area): switch to active (edit) mode.
- Icon shown in mode pill: path-outline icon when inactive, path-filled icon when active.

**Active state** — rendered as text input:
- Triggers: click bar background, `Ctrl+L`, `Alt+D`.
- Input pre-filled with current path string.
- Autocomplete suggestions dropdown:
  - Uses `nucleo` fuzzy matching against filesystem paths.
  - Shows up to 8 suggestions at a time.
  - Each suggestion: folder name text, path in secondary color.
  - Arrow keys navigate, Tab completes, Enter navigates, Escape reverts.
- Clear `✕` button inside field, right side.
- Commit: press Enter → validate path → if valid navigate, if invalid show red border + tooltip.
- Revert: press Escape or click outside → restore breadcrumb, original path.

#### Mode B: Search (`OmnibarSearchMode`)
- Triggered by: clicking the search mode icon, `Ctrl+F`, or `Ctrl+E`.
- Placeholder: "Search current folder".
- Search is performed against the current folder's items in real-time (debounced 150ms).
- Backend: `SearchService::filter_items()` with `nucleo`.
- For global search (recursive): launches a separate `SearchAction` that streams results as they arrive.
- Results view: replaces file list with a flat table showing: icon (16px), name, parent folder path, size, date modified.
- Clearing the search box or pressing Escape returns to normal folder view.
- No search = all items shown. Partial search = filtered items shown.

#### Mode C: Command Palette (`OmnibarCommandPaletteMode`)
- Triggered by: `Ctrl+Shift+P` or `OpenCommandPaletteAction`.
- Auto-focuses on open.
- Placeholder: "Type a command".
- Lists all registered actions from the `ActionRegistry` filtered by query text.
- Each row layout:
  ```
  [16px icon] [Primary text with matched chars bold] [HotKey badge right-aligned]
  ```
- `UpdateTextOnSelect = false` — selecting an item executes it, doesn't paste its name.
- Pressing Enter on selected item executes the action.
- Pressing Escape closes the palette.

### 5.3 Right Button Strip
Positioned right of the Omnibar, spacing 4px between items. When items are present, margin-right = 4px.

| Button | Size | Visibility | Description |
|---|---|---|---|
| Shelf pane toggle | 36×32 `ToggleButton` | only if shelf enabled in settings | Toggle shelf staging area |
| Status Center | 36×32 `Button` | always when any op is active | Click opens `StatusCenter` flyout (400px wide, 120–500px tall). Shows `ProgressRing` during active ops, `InfoBadge` showing count. Badge color: success green / warning yellow / error red |
| Update available | 36×32 `Button` | only when update detected | Click downloads/installs update. Shows `ProgressRing` while downloading. |

---

## 6. Sidebar

The sidebar is implemented via `NavigationViewItemButtonStyle.xaml` with a custom scroll view. Width: **250px default**, resizable **180px–500px** by dragging the right splitter edge.

### 6.1 Section Structure & Visibility Rules

Sections are shown **only if they have content**. Empty sections are hidden entirely.

```
╔════════════════════════════════╗
║  [Section: Favorites]          ║  always shown (Home is always pinned)
║    🏠  Home                    ║
║    📌  Desktop                 ║
║    📌  Downloads               ║
║    📌  Documents               ║
║    📌  Pictures                ║
║    📌  <user-pinned folder>    ║  0 or more user pins
╠════════════════════════════════╣
║  [Section: File Tags]          ║  hidden if no tags exist
║    🔴  Work                    ║
║    🔵  Personal                ║
╠════════════════════════════════╣
║  [Section: Libraries]          ║  hidden if Libraries not shown in config
║    📚  Documents               ║
║    🎵  Music                   ║
║    📷  Pictures                ║
║    🎬  Videos                  ║
╠════════════════════════════════╣
║  [Section: This PC]            ║  always shown
║    💻  Local Disk (C:) [bar]   ║
║    📀  Data (D:)       [bar]   ║
║    🔌  USB Drive (E:)  [bar]   ║
╠════════════════════════════════╣
║  [Section: Cloud Drives]       ║  hidden if none detected
║    ☁   OneDrive                ║
║    ☁   Google Drive            ║
║    ☁   Dropbox                 ║
╠════════════════════════════════╣
║  [Section: Network]            ║  hidden if no network locations
║    🖧   \\server\share          ║
╠════════════════════════════════╣
║  [Section: WSL]                ║  hidden if WSL not installed
║    🐧  Ubuntu-22.04            ║
║    🐧  Debian                  ║
╚════════════════════════════════╝
```

### 6.2 Item Visual Anatomy

Each sidebar item row is **32px tall** and follows this structure:
```
[ 8px ] [ 16px icon ] [ 8px gap ] [ label text flex ] [ optional: badge / count ] [ 8px ]
```
- **Icon**: 16×16. Shell icon for user folders; imageres.dll index for special folders; drive-type icon for volumes; colored dot for tags.
- **Label**: 13px Regular, `TextFillColorPrimary`, single line, truncate with `…`.
- **Badge/count**: e.g., recycle bin shows item count as small `TextFillColorSecondary` text on right.
- **Capacity bar** (drives only): 2px tall, full width of item, shown **below** the label text. Fill color: accent when ≤ 90% full, `SystemFillColorCritical` when > 90% full. Background: `DividerStrokeColorDefault`.

### 6.3 Section Headers
- Font: 11px SemiBold, `TextFillColorSecondary`.
- Case: **NOT uppercase** in Files-community (contrary to common assumption — they are title case: "Favorites", "This PC").
- Top padding: 8px above header, 4px below header.
- No click action on the header itself (not collapsible in the default style, though this is a nice future feature).
- Divider: 1px `DividerStrokeColorDefault` line runs from left edge of header text to right edge of sidebar.

### 6.4 State Colors for Sidebar Items
```
Default     → transparent bg
Hover       → #0FFFFFFF (SubtleFillColorSecondary)
Pressed     → #18FFFFFF (SubtleFillColorTertiary)
Active/open → #18FFFFFF bg + 2px left accent-color border (no rounded corners on that left side)
```
When the active item's path matches the current tab's directory, it is highlighted.

### 6.5 Drag & Drop on Sidebar Items
- **Pin a folder**: Drag any folder from the file list and drop onto the Favorites section → pin it. Animation: item slides into place.
- **Reorder pins**: Drag a pinned item up or down. A translucent insertion indicator line shows the drop position. Drop → reorder and persist.
- **Drag files onto sidebar item**: Hovering dragged files over a sidebar folder for **1300ms** opens/navigates to that folder. This matches `DragAndDrop.HoverToOpenTimespan = 1300`.
- **Drop onto sidebar item**: Dropping files onto a folder sidebar item moves/copies them (respects Ctrl = copy, no modifier = move).

### 6.6 Context Menu on Sidebar Items

**For pinned folders (Favorites)**:
1. Open
2. Open in New Tab
3. Open in New Window
4. Open in New Pane
5. --- separator ---
6. Unpin from Sidebar
7. Copy Path
8. Properties

**For drives (This PC)**:
1. Open
2. Open in New Tab
3. Open in New Window
4. --- separator ---
5. Copy Path
6. Format Drive
7. Open in Storage Sense
8. Properties

**For tags**:
1. Open All Tagged Items
2. --- separator ---
3. Edit Tag (rename / recolor)
4. Delete Tag

### 6.7 Drive Detection Backend
```rust
#[derive(Debug, Clone)]
enum DriveType { Unknown, Removable, Fixed, Network, CdRom, RamDisk }

struct DriveInfo {
    root: FilesPath,          // "C:\\"
    label: String,            // "Local Disk"
    drive_type: DriveType,
    total_bytes: u64,
    free_bytes: u64,
    file_system: String,      // "NTFS", "exFAT", etc.
    icon: DriveIconSource,    // imageres.dll index
}

fn enumerate_drives() -> Vec<DriveInfo> {
    // GetLogicalDriveStringsW → split on \0 → for each root:
    //   GetDriveTypeW → DriveType
    //   GetDiskFreeSpaceExW → free_bytes, total_bytes
    //   GetVolumeInformationW → label, file_system
    //   SHGetFileInfoW(root, SHGFI_ICON) → icon
}

// Hot-swap: register WM_DEVICECHANGE with DBT_DEVICEARRIVAL / DBT_DEVICEREMOVECOMPLETE
// On event: re-enumerate drives, update sidebar model, notify UI
```

### 6.8 Cloud Drive Detection
```rust
fn detect_cloud_drives() -> Vec<CloudDriveInfo> {
    let mut drives = Vec::new();

    // OneDrive: HKCU\Software\Microsoft\OneDrive → "UserFolder" value
    // Google Drive: HKCU\Software\Google\DriveFS → "PerAccountPreferences\*\mount_point_path"
    // Dropbox: HKCU\Software\Dropbox\ks\dropbox_path or AppData\Local\Dropbox\info.json
    // iCloud: %LocalAppData%\Apple Computer\CloudDocs\

    drives
}
```

### 6.9 WSL Detection
```rust
fn detect_wsl_distros() -> Vec<WslDistro> {
    // HKCU\Software\Microsoft\Windows\CurrentVersion\Lxss
    // Each subkey is a distro GUID. Read:
    //   DistributionName → display name ("Ubuntu-22.04")
    //   BasePath → \\wsl$\<name>  (UNC network path)
    // Icon: match name to known assets (alpine, debian, ubuntu, kali, opensuse)
    // Fallback icon: generic penguin (genericpng.png)
}
```

### 6.10 Sidebar Collapse Animation
- Duration: 200ms.
- Easing: `CubicEaseOut`.
- The sidebar slides left (or fades out on very narrow windows).
- Content area expands to fill the vacated space simultaneously.
- When collapsed, a thin 4px accent-colored strip remains on the left edge to indicate the sidebar is hidden (clicking it reopens it).
- Persist `sidebar_visible: bool` and `sidebar_width: f32` in config.

---

## 7. Toolbar

The Toolbar (`Toolbar.xaml`) sits between the NavBar and the file list. **User-configurable**: can be hidden entirely via `ToggleToolbar` action or Settings → Appearance. Height: **40px**, left/right padding: **4px**, background: semi-transparent (`CardBackgroundFillColorDefault`).

### 7.1 Default Button Layout

Buttons are grouped by category, separated by 1px vertical `DividerStrokeColorDefault` dividers. Each group has **4px margin** between its buttons.

```
[New Folder][New File] | [Cut][Copy][Paste][Copy Path] | [Rename][Delete] | [↔ view toggle ↔] | [⊟ Preview][ℹ Info] | [...overflow]
```

### 7.2 Individual Button Specifications

All toolbar buttons use `ToolbarButtonStyle` (from `ToolbarButtonStyle.xaml`):
- Width: **auto** (content-sized), min-width 32px.
- Height: **32px**.
- Icon: 16×16 `ThemedIcon` or `FontIcon` glyph.
- Label: optional text label (shown when toolbar is in "Icon + Text" mode).
- Tooltip: `"Label (HotKey)"` format, e.g. `"New Folder (Ctrl+Shift+N)"`.
- Hover: `ButtonBackgroundPointerOver`.
- Pressed: `ButtonBackgroundPressed`.
- Disabled: `ButtonBackgroundDisabled = Transparent`, 40% opacity.

| Button | Icon Glyph | Hotkey | Availability | Overflow priority |
|---|---|---|---|---|
| New Folder | `E8F4` | Ctrl+Shift+N | always | 1 (never overflow) |
| New File | `E7C3` | Ctrl+Shift+. | always | 2 |
| Cut | `E8C6` | Ctrl+X | items selected | 3 |
| Copy | `E8C8` | Ctrl+C | items selected | 4 |
| Paste | `E77F` | Ctrl+V | clipboard has items | 5 |
| Copy Path | `E8C8`+badge | Ctrl+Shift+C | items selected | 6 |
| Rename | `E8AC` | F2 | exactly 1 item selected | 7 |
| Delete | `E74D` | Delete | items selected | 8 |
| View Toggle | segmented | — | always | 9 (never overflow) |
| Preview Pane | `E8A0` | Ctrl+P | always | 10 |
| Info Pane | `E946` | — | always | 11 |

### 7.3 View Mode Toggle (Segmented Control)
Three segments, mutually exclusive:
- Details (list icon): `E8FD`
- Tiles/Grid (grid icon): `E80A`
- Columns (columns icon): custom SVG

Pressing a segment switches the current tab's view mode and persists per-folder in `LayoutPreferences`.

### 7.4 Overflow Button (`...`)
- Positioned at far right of toolbar.
- Glyph: `E712` (More).
- When toolbar is too narrow to show all buttons, the lowest-priority buttons move into a `MenuFlyout` that appears on click.
- The flyout shows the same buttons as full-size `MenuFlyoutItem` rows.
- This is determined by measuring available width minus fixed-width elements.

### 7.5 Toolbar Customization
- Users can show/hide individual buttons via Settings → Appearance → Toolbar.
- A drag-and-drop customization dialog (`ToolbarCustomizationPage`) allows reordering.
- The `ShowToolbar` toggle hides/shows the entire bar (preserved in config).
- The "Reset to defaults" button restores the default button set and order.

---

## 8. File List — View Modes

The file list is the core content area. All views share:
- Virtualized rendering using GPUI's `List` or `UniformList` — only visible rows are rendered. This is **mandatory** for directories with 100k+ items.
- A single unified `DirectoryLoader` produces items as a `Stream<Item=StorableItem>`. Items are batched into the UI in groups of 100.
- Selection state is maintained in `SelectionState` (set of indices or paths). It is shared across all view modes for the same pane.
- Keyboard navigation works in all views.

### 8.1 Details View

The Details view (`DetailsLayoutPage.xaml`) is a virtualized list with resizable column headers.

#### Column Header Row
- Height: **28px**.
- Background: `CardBackgroundFillColorDefault`.
- Each header cell:
  - Click = sort by that column (toggle asc/desc on second click).
  - Drag right edge = resize column (min width: 50px).
  - Right-click = context menu to show/hide columns.
  - Sort indicator: small triangle `▲` or `▼` after column label. `TextFillColorSecondary`, 10px.
  - Active sort column header: `TextFillColorPrimary` (slightly brighter).

#### Default Columns (in order)
| Column | Default Width | Min Width | Sortable | Notes |
|---|---|---|---|---|
| Name | flex (fill) | 100px | ✓ | Icon 16px left of text; inline rename; extension hidden if setting off |
| Date Modified | 180px | 120px | ✓ | Format from settings (e.g. "Apr 25, 2026 1:30 PM") |
| Type | 120px | 80px | ✓ | e.g. "File folder", "PNG image", "Text document" |
| Size | 80px | 60px | ✓ | Right-aligned, human-readable ("4.2 MB"), blank for folders by default |

#### Optional Columns (user-togglable)
- Date Created, Date Accessed, Date Deleted (recycle bin), Tags (colored dot chips), Path, Sync Status (cloud icon), Original Path (recycle bin), Git Status.

#### Row Anatomy
- Height: **36px** (normal) or **28px** (compact). Configured per-folder.
- Left edge: optional 20px checkbox zone (visible on hover or when `ShowCheckboxesWhenSelectingItems`).
- State colors:
  ```
  Default:  transparent
  Hover:    #0FFFFFFF
  Selected: #18FFFFFF + 2px left accent border (accent = system color)
  Selected+Hover: slightly lighter than selected
  Cut (for move): opacity 0.4 (DimItemOpacity)
  Hidden file: opacity 0.4
  ```
- **Inline rename**: When F2 is pressed on a selected item, the name cell replaces with a `TextInput`. The filename is pre-filled; the extension is NOT selected (cursor placed before extension). Pressing Enter commits. Pressing Escape cancels. Invalid char input (`\/:*?"<>|`) shows a tooltip error. If the name already exists, show conflict prompt.

#### Natural Sort
File names are sorted using a **natural string comparer** (numbers in strings sorted numerically):
- "File 2" comes before "File 10" ✓  
- "img_100" comes after "img_20" ✓  
Implementation: the `NaturalStringComparer.cs` uses `CompareStringEx` Win32 API with `SORT_DIGITSASNUMBERS` flag. Use `windows-rs` equivalent.

### 8.2 Grid / Tiles View

The Grid view (`GridLayoutPage.xaml`) shows files as icon+label cards in a wrapping grid.

#### Layout
- Item cell: **100px wide × 120px tall** (default; can be resized by user in settings).
- Gap between items: **4px** horizontally and vertically.
- Items wrap left-to-right, then new row.
- All items in a row are the same height (uniform grid, not masonry).

#### Item Anatomy
```
┌──────────────────────┐  ← 100px wide, 120px tall
│                      │
│   [ 64px thumbnail ] │  ← centered, 10px from top
│                      │
│  Name line 1         │  ← 12px Regular, center-aligned
│  Name line 2…        │  ← 2nd line if needed, then ellipsis
└──────────────────────┘
```
- **Thumbnail**: 64×64px. Try shell `IThumbnailProvider` → `IExtractImage` → extension icon fallback.
- **Name**: max 2 lines of 12px text, center-aligned, `TextFillColorPrimary`. Truncate line 2 with `…`.
- **State backgrounds**: rounded-rect fill (radius 4px):
  - Hover: `#0FFFFFFF`
  - Selected: `accent@20%` fill + 2px `accent` outline
  - Selected+Hover: `accent@30%`
- **Hidden items**: full cell opacity = 0.4.

#### Tile Size Variants
- Small: 80×90px, 32px icon.
- Medium (default): 100×120px, 64px icon.
- Large: 160×180px, 96px icon.
- Extra Large: 220×240px, 128px icon.

User selects from these presets via right-click → View → Size submenu or toolbar slider.

### 8.3 Columns View (Miller Columns / macOS Finder-style)

Implemented in `ColumnsLayoutPage.xaml`. A horizontally-scrollable strip of independent pane columns.

- Each column: **240px wide** (fixed), full-height.
- Each column shows the contents of a single directory as a compact Details View (Name + Type columns only).
- Selecting a folder in column N automatically loads its contents in column N+1.
- Selecting a file in any column shows its preview in the rightmost column (preview pane embedded here, not separate).
- **Scroll**: the strip scrolls horizontally when the number of columns exceeds the window width.
- The currently-active path is highlighted (active column has a slightly brighter background).
- Breadcrumb in NavBar stays in sync with the deepest active column.

### 8.4 Cards View
- Renders items as rich, distinct card components.
- Each card contains an enlarged thumbnail/icon, with structured metadata (Name, Size, Type, Tags) stacked vertically or horizontally beside it depending on window width.

### 8.5 Adaptive View
- Dynamically selects the most appropriate layout mode (Details, Grid, List) based on the current directory's content.
- Example: Switches to Grid View for folders primarily containing images; switches to Details View for source code or document directories.

### 8.6 Sorting

Sort options (from `SortAction.cs`):
```
Name, DateModified, DateCreated, Size, FileType, Path, OriginalPath,
DateDeleted, SyncStatus, FileTag
```
Sort direction: Ascending / Descending (toggle).

**Folders/Files ordering** (three mutually exclusive modes):
- Folders first (default)
- Files first
- Interleaved (directories sorted alongside files alphabetically)

### 8.7 Per-Folder Layout Persistence
- **Behavior**: The application tracks layout mode (Grid vs Details), Sort field, Sort direction, and Group-by settings on a per-directory basis using a SQLite database (`LayoutPreferencesDatabaseManager`).
- **Global Fallback**: If no specific layout is saved for a path, the app falls back to the globally configured default view settings.

**Group by** options:
```
None, Name (letter A-Z), DateModified (Today/Yesterday/This Week/…),
DateCreated, Size (bucket ranges), FileType, SyncStatus, FileTag,
OriginalPath, DateDeleted
```
Groups can be **collapsed** (chevron UI on group header row).

Sort and group settings are **per-folder** (stored in `LayoutPreferences` alongside folder path). Optionally: sync across all folders via `SyncFolderPreferencesAcrossDirectories` setting.

### 8.5 Selection Mechanics

- **Single click**: select item (deselect previous).
- **Ctrl+click**: toggle selection of item (multi-select).
- **Shift+click**: range-select from last selected to clicked item.
- **Rubber-band (lasso)**: click-drag on empty space draws a selection rectangle. Items whose bounds intersect it become selected.
- **Select all**: Ctrl+A.
- **Invert selection**: all unselected become selected, all selected become unselected.
- **Clear selection**: Escape or click empty space.
- **Checkbox mode**: when `ShowCheckboxesWhenSelectingItems` is on, clicking anywhere on the row (not the checkbox) selects it; clicking the checkbox toggles it. Allows building a selection without holding Ctrl.
- **Select on hover**: when enabled, hovering over an item for a short delay selects it (useful for touch/accessibility). Off by default.

### 8.6 Grouping Header Rows

When `GroupBy` is set, group header rows appear between groups:
- Height: 36px.
- Background: `CardBackgroundFillColorDefault`.
- Left: `▼` chevron (click to collapse/expand group) + group name text (13px SemiBold).
- Right: item count in group (12px `TextFillColorSecondary`).
- Sticky: group headers remain visible as you scroll through the group (pinned at top).
- Collapsed group: shows only the header row; items hidden.

### 8.7 Empty State Indicators

- **Empty folder**: centered vertically in content area. Large folder icon (64px, semi-transparent), then "This folder is empty" (14px `TextFillColorSecondary`).
- **No search results**: same layout but "No items match your search" and a smaller search icon.
- **Permission denied**: "You don't have permission to view this folder" with a shield icon.
- **Loading**: a subtle `ProgressRing` in the center while the directory is loading. Disappears as soon as the first batch of items arrives.

---

## 9. Status Bar

The StatusBar (`StatusBar.xaml`) sits at the very bottom of the content column, above nothing. Height: **32px**. Padding: `8px left, 0 top/bottom`. Background: `CardBackgroundFillColorDefault` (matches toolbar).

### 9.1 Layout (3-column grid)
```
[LEFT: info text — flex-grow] [CENTER: git action buttons] [RIGHT: branch button]
```

### 9.2 Left Section — Folder & Selection Info

Items display in sequence, each separated by a 1px × 18px `DividerStrokeColorDefault` vertical line:

| Segment | Example | Visibility |
|---|---|---|
| Directory item count | "123 items" | always |
| Selected count | "3 selected" | only when items selected |
| Selected total size | "4.2 MB" | only when files selected (not folders — folders are async) |

- Font: 12px Regular, `TextFillColorSecondary`.
- Folder sizes: not shown by default for folder selections (computing recursively is expensive). When `CalculateFolderSizes` setting is on, async compute and show when ready.
- Divider: `Border` element, width 1, height 18, background `DividerStrokeColorDefault`.

### 9.3 Center Section — Git Action Buttons

Only rendered when current directory is inside a valid git repository.

**"Open in IDE" button**:
- Height: 24px, padding `8px 0`.
- Background: transparent / border transparent.
- Content: 16px IDE icon + "Open" text label.
- Flyout:
  - "Open Folder in IDE" → `OpenInIDE` action.
  - "Open Repository in IDE" → `OpenRepoInIDE` action (only if git branch detected).

**Git network status button** (only if branch tracked remote):
- Height: 24px, padding `8px 0`.
- Content: 16px git icon + status string e.g. `↑2 ↓1` or `↑0 ↓3`.
- Tooltip: extended status info ("2 commits ahead, 1 behind origin/main").
- Flyout contains: Pull, Push, Sync (Fetch+Pull) menu items, each with their icon and hotkey.

**Separator** between IDE button and network button: 1px × 18px divider.

### 9.4 Right Section — Git Branch Button

Only rendered when `StatusBarViewModel.GitBranchDisplayName` is non-null.

- Height: 24px, padding `8px 0`, `margin-left: 4px`.
- Background: transparent / border transparent.
- Content: 16px branch icon (`App.ThemedIcons.Git.Branch`) + branch name text.
- Tooltip: "Manage Branches".
- Click → Branch management flyout:
  - Size: 300px wide × 340px tall.
  - Header: segmented "Locals | Remotes" tab + "Create Branch" button (right-aligned, 24px, 12px font).
  - Body: scrollable `ListView` of branches. Each row:
    - Branch name text.
    - Delete button (trash glyph `E74D`, 12px) — only for local non-head non-remote branches.
  - Active branch highlighted.
  - Clicking a branch: switch to it (git checkout).

### 9.5 Advanced GitHub Integrations
- **GitHub Login**: Initiates OAuth flow via `GitHubLoginDialog` to authenticate the user and retrieve a token for remote operations.
- **Clone Repository**: `CloneRepoDialog` allows users to paste a remote Git URL and select a local destination directory for `git clone`.
- **Branch Management**: `AddBranchDialog` prompts the user for a new branch name and automatically checks it out.

---

## 10. Panes & Windowing Layouts

### 10.1 Dual Pane (Split Pane) Layout
- **Behavior**: A single tab can be split horizontally or vertically (`SplitPaneHorizontally`, `SplitPaneVertically`), creating two independent navigation boundaries within the same window.
- **Interaction**: Files can be dragged and dropped between panes seamlessly.

### 10.2 Preview Pane (right side, ~320px, resizable)
Toggle with button in toolbar or `Ctrl+P`.  
Shows live preview of selected item:

| File type | Renderer |
|-----------|----------|
| Images (jpg, png, gif, webp, svg) | `resvg` / raw decode via `image` crate |
| Text, code, markdown | syntax-highlighted text (char limit 10,000) |
| PDF | first 10 pages via `pdfium-render` or similar |
| HTML | system WebView2 embed |
| Audio/Video | system media player embed |
| Folder | thumbnail grid of first N children |
| Other | shell `IPreviewHandler` COM interop |

### Info Pane (right side, same space, tabbed with Preview)
Shows properties of selected item:
- Name, full path, type, size (folders: calculated async).
- Created, Modified, Accessed timestamps.
- Tags (editable chips).
- EXIF data for images (dimensions, camera, GPS).
- Git status + last commit if in repo.
- Security: owner, permissions summary.

---

## 11. Home Page

Shown when path = "Home" (app startup default).  
Full content area replaced by widget grid.

### Widgets
**Quick Access** — pinned folders + frequently visited  
- 2-column card grid. Each card: 48px folder icon, name, last-accessed time.

**Drives** — one card per mounted volume  
- Drive letter, label, type icon, capacity bar (red if >90% full).
- Buttons: Open, Format, Storage Sense.

**Recent Files** — Windows Shell recent items  
- Table list: icon, name, path, date accessed.
- Source: `SHGetKnownFolderPath(FOLDERID_Recent)`.

**File Tags** — grouped by tag color  
- Tag chip (colored circle + name), count of tagged files.
- Click → filter to all tagged files.

**Network Locations** — SMB shares and network drives  
- Cards similar to Drives widget.

---

## 12. Context Menu (Right-click)

Primary items (always shown, 48px tall):
- Open, Open With, Open in New Tab, Open in New Pane, Open in New Window

Secondary items (32px tall, shown below separator):
- Cut, Copy, Copy as Path, Paste (if clipboard has files)
- Rename, Delete, Delete Permanently
- Create Shortcut, Compress, Extract Here
- Tags submenu (color dots for each tag)
- Share
- Properties

Shell extension items injected at bottom via `IContextMenu` COM.

**Sizing**: max height 480px, overflow scrollable. Item width max 250px.

---

## 12.1 Smart Tags Context Menu
The `FileTagsContextMenu` provides a reactive interface for managing item tags.

### Behavioral Logic
- **Selection State Tracking**: The menu must track the tags applied to the currently selected item(s).
- **Visual Indicators**:
  - **Checked**: Tag is applied to ALL selected items.
  - **Indeterminate (Dash)**: Tag is applied to SOME but not all selected items.
  - **Unchecked**: Tag is applied to NONE of the selected items.
- **Toggling Logic**:
  - Clicking an **Unchecked** or **Indeterminate** tag applies it to the entire selection.
  - Clicking a **Checked** tag removes it from the entire selection.
- **Dynamic Updates**: When a tag is toggled, the menu should emit a `TagsChanged` event to refresh the UI (e.g., updating the tag column in Details view and the Info pane).

## 13. Dialogs

All dialogs use:
- Rounded corners (radius: 8px system overlay).
- Mica or Acrylic background.
- Title (20px SemiBold) + body content + footer button row (primary right, cancel left).

### File Operation Progress Dialog
- Title: "Copying 3 items to Documents"
- Progress bar: determinate % or indeterminate.
- Speed graph: `SpeedGraph` canvas (rolling 60-point line chart, 400px wide).
- Current file: truncated path text.
- ETA and speed: "4.2 MB/s — 1 minute remaining".
- Pause / Cancel buttons.
- Conflict section: Replace / Skip / Rename for each conflicting file.

### Rename Dialog (inline)
- In-place text field replacing the name label in the file list.
- Pre-selects name without extension.
- Enter = commit, Escape = cancel.
- Validates: no invalid chars (`\/:*?"<>|`), no empty name.

---

## 14. Backend Logic Contracts

### 14.1 Action System
Every user-triggerable operation is an `Action`. All actions live in a central `ActionRegistry` keyed by ID string, enabling the command palette and hotkey remapping.

```rust
trait Action: Send + Sync + 'static {
    fn id(&self) -> &'static str;              // unique, e.g. "files.copy"
    fn label(&self) -> String;                 // localized display name
    fn description(&self) -> String;           // longer tooltip description
    fn hotkeys(&self) -> Vec<KeyStroke>;       // default bindings (rebindable)
    fn icon_glyph(&self) -> char;              // Segoe MDL2 codepoint
    fn is_available(&self, ctx: &ActionContext) -> bool;
    fn execute(&self, ctx: &mut ActionContext, cx: &mut App);
}

// Toggle actions (e.g. ShowHiddenItems, ToggleSidebar) additionally implement:
trait ToggleAction: Action {
    fn is_on(&self, ctx: &ActionContext) -> bool;
}

struct ActionContext<'a> {
    selected_items: &'a [StorableItem],
    current_path: &'a FilesPath,
    clipboard: &'a Clipboard,
    pane_count: usize,
    is_git_repo: bool,
    active_pane_index: usize,
}

struct ActionRegistry {
    actions: HashMap<&'static str, Box<dyn Action>>,
    hotkey_map: HashMap<KeyStroke, &'static str>,  // keymap lookup
}
impl ActionRegistry {
    fn register(&mut self, action: impl Action);
    fn by_hotkey(&self, key: KeyStroke) -> Option<&dyn Action>;
    fn search(&self, query: &str) -> Vec<&dyn Action>;  // for command palette
}
```

### 14.2 Navigation History (per-pane)
```rust
struct NavigationHistory {
    back_stack: VecDeque<HistoryEntry>,   // max 50
    forward_stack: VecDeque<HistoryEntry>,
    current: HistoryEntry,
}

struct HistoryEntry {
    path: FilesPath,
    scroll_offset_px: f32,    // restore scroll position on back/forward
    selection: Vec<FilesPath>, // restore selection
}

impl NavigationHistory {
    fn navigate(&mut self, path: FilesPath) {
        // push current → back_stack, clear forward_stack, set current
    }
    fn back(&mut self) -> Option<&HistoryEntry>;
    fn forward(&mut self) -> Option<&HistoryEntry>;
    fn jump_back_to(&mut self, index: usize); // for history flyout direct jump
    fn back_entries(&self) -> &[HistoryEntry]; // for history flyout display
}
```

### 14.3 File Operation Engine
Operations run fully on `tokio` background tasks. Progress is reported via `tokio::sync::mpsc` to the UI layer. The UI updates `StatusCenter` on each progress tick.

```rust
#[derive(Clone)]
enum OperationKind { Copy, Move, Delete, Rename }

#[derive(Clone)]
enum ConflictResolution { Replace, Skip, RenameNew, RenameExisting }

#[derive(Clone)]
struct Conflict {
    source: FilesPath,
    destination: FilesPath,
    resolution: Option<ConflictResolution>, // None = awaiting user
}

struct FileOperation {
    id: Uuid,
    kind: OperationKind,
    sources: Vec<FilesPath>,
    destination: Option<FilesPath>,
    status: OperationStatus,       // Queued, Running, Paused, Done, Failed
    bytes_total: u64,
    bytes_done: u64,
    speed_bps_ring: VecDeque<u64>, // last 60 samples for speed graph
    eta_secs: Option<u64>,
    conflicts: Vec<Conflict>,
    error: Option<String>,
}

impl FileOperation {
    fn progress(&self) -> f32 {
        if self.bytes_total == 0 { return 0. }
        self.bytes_done as f32 / self.bytes_total as f32
    }
    fn speed_bps_avg(&self) -> u64 {
        // rolling average of last 60 speed samples
    }
}
```

Operations Manager in `StatusCenterService`:
```rust
struct StatusCenterService {
    ops: Arc<Mutex<Vec<FileOperation>>>,
    tx: mpsc::Sender<OperationEvent>,
}
enum OperationEvent {
    Progress { id: Uuid, bytes_done: u64, speed_bps: u64 },
    Conflict { id: Uuid, conflict: Conflict },
    Done { id: Uuid },
    Error { id: Uuid, message: String },
}
```

### 14.4 Directory Loader (Cancellable Streaming)
```rust
struct DirectoryLoader;

impl DirectoryLoader {
    // Returns a stream that can be cancelled mid-flight when user navigates away.
    // Uses jwalk for parallel directory reading.
    // Items arrive in batches; UI re-renders on each batch.
    pub async fn load(
        path: FilesPath,
        cancel: CancellationToken,
        config: &AppConfig,
    ) -> impl Stream<Item = Vec<StorableItem>> {
        // jwalk::WalkDir::new(&path).into_iter()
        // filter: if !config.show_hidden → skip hidden items
        // filter: if !config.show_system → skip system items
        // yield batches of 100 items
        // check cancel.is_cancelled() between batches
    }
}
```

### 14.5 File Watcher → Live Refresh
```rust
// storage.rs FileWatcher (notify crate) sends events:
enum FileEvent {
    Created(FilesPath),
    Deleted(FilesPath),
    Modified(FilesPath),
    Renamed { from: FilesPath, to: FilesPath },
}

// In AppWindow (GPUI entity):
// - Hold a tokio::sync::mpsc::Receiver<FileEvent>
// - On each GPUI frame tick, drain up to 64 events
// - Debounce: accumulate events for 100ms then apply:
//     Created → insert item into list at correct sorted position
//     Deleted → remove item from list
//     Modified → update metadata for that item
//     Renamed → update name in-place
// - Call cx.notify() to trigger re-render only if events were processed
```

### 14.6 Sort & Group Engine
```rust
#[derive(Clone, Serialize, Deserialize)]
struct SortState {
    column: SortColumn,
    direction: SortDir,             // Asc, Desc
    folder_order: FolderOrder,      // FoldersFirst, FilesFirst, Interleaved
    group_by: Option<GroupColumn>,
    collapsed_groups: HashSet<String>,
}

enum SortColumn {
    Name, DateModified, DateCreated, DateDeleted, Size,
    FileType, Path, SyncStatus, FileTag, OriginalPath,
}

// Sorting is purely in-memory after load:
fn sort_items(items: &mut Vec<StorableItem>, state: &SortState) {
    items.sort_by(|a, b| {
        // 1. folder_order: maybe put folders before files
        // 2. NaturalStringCompare if SortColumn::Name
        // 3. else numeric/chronological compare
        // 4. apply SortDir
    });
}

fn group_items(items: &[StorableItem], state: &SortState) -> Vec<Group> {
    // Partition into groups based on state.group_by
    // e.g. GroupColumn::DateModified → "Today", "Yesterday", "This Week", etc.
}
```

### 14.7 Thumbnail & Icon Cache
```rust
struct IconCache {
    // Keyed by lowercase extension ("png", "rs", "exe")
    by_extension: DashMap<String, Arc<ImageData>>,
    // Keyed by full path for special folders and exact thumbnails
    by_path: DashMap<FilesPath, Arc<ImageData>>,
    lru: Mutex<LruCache<CacheKey, ()>>,  // max 1000 entries total
}

impl IconCache {
    // Called from GPUI render thread — returns immediately with cached
    // result or a placeholder, schedules async load if not cached.
    fn get_or_schedule(&self, item: &StorableItem, size: IconSize) 
        -> Arc<ImageData>;

    async fn load_shell_icon(path: &FilesPath, size: IconSize) 
        -> Result<ImageData> {
        // On COM STA thread:
        // 1. Try IThumbnailProvider::GetThumbnail(size)
        // 2. Fall back to IExtractImage::GetLocation + Extract
        // 3. Fall back to SHGetFileInfoW SHGFI_ICON + SHGFI_ICONLOCATION
        // 4. Fall back to generic icon by type
        // Convert HBITMAP/HICON → raw BGRA bytes → ImageData
    }
}

enum IconSize { Small = 16, Medium = 32, Large = 48, ExtraLarge = 256 }
```

### 14.8 Settings Reactivity
```rust
// AppConfig is wrapped in an Arc<RwLock<>> for shared read access.
// All mutations go through ConfigService which also fans out change events.

struct ConfigService {
    config: Arc<RwLock<AppConfig>>,
    // Weak references to views that subscribe to changes
    change_tx: broadcast::Sender<ConfigChange>,
}

enum ConfigChange {
    ShowHiddenFiles(bool),
    ShowFileExtensions(bool),
    Theme(ThemeMode),
    DefaultViewMode(ViewMode),
    // ... one variant per meaningful setting
}

impl ConfigService {
    fn update<F: FnOnce(&mut AppConfig)>(&self, f: F) {
        { let mut c = self.config.write(); f(&mut c); }
        self.save_to_disk();
        // change_tx.send(...) — each open pane receives and re-filters
    }
}
```

### 14.9 Recycle Bin Integration
```rust
// Special path constant:
const RECYCLE_BIN_PATH: &str = "::{645FF040-5081-101B-9F08-00AA002F954E}";

// When navigating to RecycleBin path:
struct RecycleBinItem {
    original_path: FilesPath,
    deleted_at: SystemTime,
    size_bytes: u64,
    display_name: String,
}

fn list_recycle_bin() -> Vec<RecycleBinItem> {
    // COM STA thread:
    // SHGetKnownFolderPath(FOLDERID_RecycleBinFolder)
    // IShellFolder::EnumObjects with SHCONTF_NONFOLDERS | SHCONTF_FOLDERS
    // IShellFolder2::GetDetailsOf for original path, date deleted, size
}

fn restore_item(item: &RecycleBinItem) -> Result<()> {
    // SHFileOperationW FO_MOVE from recycle bin PIDL to original_path.parent()
}

fn empty_recycle_bin(confirm: bool) -> Result<()> {
    // SHEmptyRecycleBinW(hwnd, nullptr, flags)
    // flags: if confirm=false → SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI
}
```

---

## 15. Icon & Asset System

### 15.1 Shell Icon Extraction Pipeline
```rust
// All shell icon calls must happen on the COM STA thread (src/windows_interop.rs).
// Results are sent back to the UI thread via oneshot channel.

async fn get_shell_icon(path: FilesPath, size: IconSize) -> Result<ImageData> {
    // Dispatch to STA thread:
    com_sta_thread::call(move || {
        let mut shfi = SHFILEINFOW::default();
        let flags = SHGFI_ICON | SHGFI_USEFILEATTRIBUTES 
                  | match size {
                        IconSize::Small => SHGFI_SMALLICON,
                        _ => SHGFI_LARGEICON,
                    };
        SHGetFileInfoW(path.as_wide(), 0, &mut shfi, flags);
        // shfi.hIcon → HICON
        // GetIconInfo(hIcon, &iconInfo) → iconInfo.hbmColor (HBITMAP)
        // GetDIBits → raw BGRA pixel data
        // DestroyIcon(hIcon)
        // Wrap bytes in ImageData { width, height, data: Vec<u8> }
    }).await
}
```

Cache strategy:
- Files with the same extension share one icon (except executables — use per-path icon).
- Folders use per-path icon (special folders differ: Desktop, Downloads, etc.).
- Thumbnails are always per-path.
- Cache is pre-warmed for visible items on directory load. Off-screen items loaded lazily.

### 15.2 Complete Segoe MDL2 Assets Glyph Table

| Action / Meaning | Glyph (hex) | Unicode char |
|---|---|---|
| Hamburger menu | `E700` | `\u{E700}` |
| Home | `E80F` | `\u{E80F}` |
| Back | `E72B` | `\u{E72B}` |
| Forward | `E72A` | `\u{E72A}` |
| Up | `E74A` | `\u{E74A}` |
| Refresh | `E72C` | `\u{E72C}` |
| Overflow more | `E712` | `\u{E712}` |
| Search | `E721` | `\u{E721}` |
| Close / Chrome close | `E8BB` | `\u{E8BB}` |
| Add / New tab | `E710` | `\u{E710}` |
| New folder | `E8F4` | `\u{E8F4}` |
| New file | `E7C3` | `\u{E7C3}` |
| Cut | `E8C6` | `\u{E8C6}` |
| Copy | `E8C8` | `\u{E8C8}` |
| Paste | `E77F` | `\u{E77F}` |
| Delete / Trash | `E74D` | `\u{E74D}` |
| Rename / Edit | `E8AC` | `\u{E8AC}` |
| Pin | `E840` | `\u{E840}` |
| Tag | `E1CB` | `\u{E1CB}` |
| Settings | `E713` | `\u{E713}` |
| Share | `E72D` | `\u{E72D}` |
| Properties | `E946` | `\u{E946}` |
| Folder | `E8B7` | `\u{E8B7}` |
| File | `E7C3` | `\u{E7C3}` |
| Sort | `E8CB` | `\u{E8CB}` |
| Filter | `E71C` | `\u{E71C}` |
| Preview pane | `E8A0` | `\u{E8A0}` |
| Details view | `E8FD` | `\u{E8FD}` |
| Grid view | `E80A` | `\u{E80A}` |
| Expand | `E8A8` | `\u{E8A8}` |
| Collapse | `E8AB` | `\u{E8AB}` |
| Undo | `E7A7` | `\u{E7A7}` |
| Redo | `E7A6` | `\u{E7A6}` |
| Upload / Move to | `E898` | `\u{E898}` |
| Download | `E896` | `\u{E896}` |
| Zip / Archive | `E8B1` | `\u{E8B1}` |
| Link / Shortcut | `E71B` | `\u{E71B}` |
| Cloud | `E753` | `\u{E753}` |
| Network | `E968` | `\u{E968}` |
| Drive | `EDA2` | `\u{EDA2}` |
| USB / Removable | `E88E` | `\u{E88E}` |
| Shield / Admin | `E8D7` | `\u{E8D7}` |
| Git (branch) | `E902` | custom |

All glyphs rendered via GPUI with font family **"Segoe MDL2 Assets"** (always available on Windows 10+).

### 15.3 Themed Icon System (Files-community Custom Icons)
Files-community also uses custom `ThemedIcon` SVG paths stored in `PathIcons.xaml` (29 KB of SVG path data). These cover:
- Omnibar mode icons (path, search, command palette)
- Status Center icon
- Shelf pane icon
- Git icons (pull, push, sync, branch)
- View layout icons (single pane, split pane arrangements)

For the Rust port: these must be converted to GPUI `Path`-based rendering or loaded as SVG via `resvg`. Store the SVG path strings in a `themed_icons.rs` constants file, one constant per icon. Render at the correct size using GPUI's vector path API.

---

## 16. Accessibility & UX Fidelity Details

### 16.1 Tooltips
Every interactive element must have a tooltip matching this format:
- Buttons with hotkeys: `"Label (HotKey)"` — e.g. `"Navigate Back (Alt+Left)"`
- Toggle buttons: `"Label"` only — the checked state is communicated by the visual state
- Toolbar buttons: label + hotkey even if label text is already shown
- Sidebar items: full path string when label might be truncated

### 16.2 Keyboard Navigation Order (Tab Order)
```
TabBar tabs → NavBar (sidebar toggle → back → fwd → up → refresh → omnibar → right buttons)
→ Toolbar (left to right)
→ Sidebar (top to bottom within the sidebar list)
→ File list (first item)
→ Preview/Info pane (if open)
→ Status bar git branch button (if visible)
```
Arrow keys navigate within the file list / sidebar without leaving the focus region.

### 16.3 Mouse Interaction Timing
| Behavior | Delay | Source |
|---|---|---|
| Hover-to-open folder while dragging | 1300ms | `DragAndDrop.HoverToOpenTimespan` |
| Select-on-hover (when enabled) | 600ms | `FolderSettings.SelectFilesOnHover` |
| Tooltip appear delay | 400ms | System default |
| Autocomplete suggestion debounce | 150ms | NavBar path mode |
| File watcher debounce | 100ms | `FileWatcher` batch window |

### 16.4 Right-Click Drag Behavior
When the user drags with the **right mouse button** and drops:
- A context menu appears at drop point:
  1. Copy here
  2. Move here
  3. Create shortcut here
  4. Cancel

### 16.5 Double-Click Behaviors
| Item type | Double-click action |
|---|---|
| Folder / directory | Navigate into folder |
| `.exe`, `.com` | Run the executable |
| `.lnk` shortcut | Follow shortcut, open target |
| Any other file | `ShellExecuteW(path, "open")` — default app |
| Archive (`.zip`, `.7z`, etc.) | Navigate into archive as virtual folder |
| Image file | Open in system viewer (or internal preview if configured) |

### 16.6 Drag Feedback Visuals
- **While dragging**: semi-transparent ghost of dragged items at cursor position. Item count badge on ghost if multiple items.
- **Valid drop target hover**: target folder/item gets a 2px accent-color outline.
- **Invalid drop target**: cursor changes to `No` symbol (🚫). No visual change on target.
- **Breadcrumb drag target**: the hovered breadcrumb segment lights up with `accent@30%` background.

### 16.7 High-DPI / Scaling
- All pixel values in this spec are at **100% DPI (96 DPI = 1x)**. At higher DPI, multiply by the scale factor.
- GPUI handles DPI scaling internally — specify all sizes in logical pixels.
- Shell icons must be requested at physical pixels: for a 16px logical icon at 150% scale, request 24px from `SHGetFileInfoW`.
- Icon cache keys must include the DPI scale factor.

### 16.8 Animation Timings
| Animation | Duration | Easing |
|---|---|---|
| Sidebar collapse/expand | 200ms | CubicEaseOut |
| Tab open (slide in) | 150ms | CubicEaseOut |
| Tab close (fade + collapse) | 150ms | CubicEaseIn |
| Item selection flash | 80ms | Linear |
| Context menu appear | 100ms | CubicEaseOut |
| Group collapse/expand | 200ms | CubicEaseInOut |
| Preview pane slide in | 250ms | CubicEaseOut |
| Status center flyout | 150ms | CubicEaseOut |

### Shell Icon Extraction
```rust
// Use windows-rs: SHGetFileInfoW with SHGFI_ICON | SHGFI_SMALLICON
// Convert HICON to GPUI ImageData via GetIconInfo + GetDIBits
// Cache by: extension for files, path for special folders
// Shell Overlays: Use `SHGFI_OVERLAYINDEX` to extract system overlays (shortcut arrow, sync checkmark, BitLocker lock) and composite them over the primary icon.
// NTFS Attributes: Detect `FILE_ATTRIBUTE_COMPRESSED` (blue arrows) and `FILE_ATTRIBUTE_ENCRYPTED` (lock icon) to apply thematic overlays.
// Tag Storage: Tags are stored in an NTFS Alternate Data Stream (ADS) named `:files`. This ensures tag portability across NTFS volumes.
// File Tracking: The app uses `System.FileFRN` (File Reference Number) to maintain a persistent link to tagged files even if they are renamed or moved within the same volume.
// Cloud Integration: Quota and sync status for cloud drives (OneDrive, iCloud, etc.) are retrieved via the Windows Cloud Filter API (`IStorageProviderStatusUISourceFactory`).
// BitLocker Status: Detect drive encryption status via `System.Volume.BitLockerProtection`. Status code `6` indicates a locked drive; trigger system unlock prompt on access.
// MTP Path Resolution: Convert `\\?\Device...` paths into shell Portable Device namespace paths to ensure thumbnail and property extraction works for mobile devices.
// Sizes: 16px (list), 32px (grid small), 48px (grid normal), 256px (jumbo)
```

### Fluent Icon Replacements (for non-shell icons)
Files-community uses Segoe MDL2 Assets glyphs. Map key glyphs:

| Meaning | Glyph codepoint |
|---------|-----------------|
| Home | `E80F` |
| Back | `E72B` |
| Forward | `E72A` |
| Up | `E74A` |
| Refresh | `E72C` |
| Search | `E721` |
| New folder | `E8F4` |
| Delete | `E74D` |
| Rename | `E8AC` |
| Cut | `E8C6` |
| Copy | `E8C8` |
| Paste | `E77F` |
| Pin | `E840` |
| Tag | `E1CB` |
| Git branch | `E902` (custom) |
| Settings | `E713` |

Render via GPUI text with "Segoe MDL2 Assets" font family at specified px size.

---

## 17. System Integration & Background Lifecycle

Files-community provides deep system integration to optionally replace the native Windows Explorer.

### 17.1 Native "New File" Menu (`ShellNew`)
- **Behavior**: Instead of hardcoding "New Text Document" or "New Folder", the application dynamically queries the Windows Registry (`HKCR\*\ShellNew` and `HKCR\*\FileName`) to populate the right-click "New" context menu with the exact same templates that the native Windows Explorer provides (e.g., "New Microsoft Word Document", "New Photoshop Image").

### 17.2 Set as Default File Manager
- **Mechanism**: Modifies the Windows Registry at `HKCU\Software\Classes\Folder\shell\open\command`.
- **Behavior**: Intercepts attempts to open folders from other applications or the Run dialog, redirecting them to `Files.App.Launcher.exe` which then forwards the request via IPC to the main application instance.
- **Settings UI**: Advanced -> "Set Files as the default file manager". Requires a UAC elevation prompt (`ElevateConfirmDialog`).

## 18. Additional Power User Operations

### 18.1 Shelf Pane (Staging Area)
- **Concept**: A non-hierarchical, persistent virtual drop-zone that stays open alongside the file list.
- **Behavior**: Users can drag and drop items from various directories into the Shelf. This does not immediately move the files. Users can later select all staged items in the Shelf to bulk-copy, bulk-move, or bulk-delete them in a single action to a final destination.

### 18.2 Advanced Context Operations
- **New Folder with Selection**: (`CreateFolderWithSelectionAction`) Select multiple files, right click, and execute this action to automatically group them into a newly created folder.
- **Copy Path with Quotes**: (`CopyItemPathWithQuotesAction`) Copies the absolute path wrapped in double quotes for easy pasting into terminal environments.

### 17.2 Replace Default File Dialogs
- **Mechanism**: Registers custom COM DLLs (`Files.App.OpenDialog` and `Files.App.SaveDialog`) to override `IFileOpenDialog` and `IFileSaveDialog`.
- **Behavior**: When any standard Windows app opens a file picker, the Files-community UI is shown instead.
- **Settings UI**: Advanced -> "Set Files as default open/save dialog". Requires UAC elevation.

### 17.3 Background Lifecycle
- **Startup**: App can be configured to launch on Windows startup via the WinRT `StartupTask` API.
- **Background Persistence**: The setting `LeaveAppRunning` keeps the application process alive in the background even when all windows are closed. This eliminates cold-start times for subsequent window opens.
- **System Tray**: When running in the background, a System Tray (Notification Area) icon is shown (`ShowSystemTrayIcon`). Clicking it opens a new window; right-clicking provides options like "Quit".

### 17.4 Third-Party Preview Integrations
- **Spacebar Trigger**: Pressing Spacebar on a selected file invokes an installed third-party quick-preview utility if available, bypassing the internal preview pane.
- **Supported Providers**: Integration implemented for **PowerToys Peek**, **QuickLook**, and **SeerPro**.
- **In-App Rich Previews**:
  - **Code**: Syntax highlighting for dozens of languages.
  - **Markdown**: Full HTML-rendered preview of `.md` files.
  - **PDF & Media**: Native handling for document formats and Video/Audio playback.

### 17.5 Native Windows OS Integrations
- **Share UI**: Triggers the native Windows `DataTransferManager` to show the OS-level Share dialog when clicking the Share button.
- **Toast Notifications**: Utilizes `AppToastNotificationHelper` to send system-level notifications for background operations (e.g. file transfer completion).

### 17.6 Custom Properties Dialog
- **Behavior**: Instead of relying solely on `ShellExecuteExW` to show the native Windows file properties dialog, Files-community uses a fully custom properties dialog window (`BasePropertiesPage`).
- **Tabs**: General, Security (ACLs), Customization (folder icons & `desktop.ini`), Hashes (MD5, SHA1, SHA256, CRC32), Compatibility (Run as Admin, OS emulation), and Signatures (Digital Certificates).
- **Fallback**: The native shell properties dialog is still invoked if the custom dialog cannot handle a specific extension or shell namespace object.

### 17.7 File Lock Detection (RestartManager)
- **Behavior**: If a file operation (delete, move) fails because the file is in use, the application uses the Win32 `RmStartSession` and `RmGetList` APIs to query exactly which background process has a lock on the file and presents the process name to the user.

### 17.8 Taskbar Progress Badges
- **Behavior**: During long-running file operations (copying, extracting), the Windows Taskbar icon will display a progress bar overlay, turning green on success or red on error/conflict using the `TaskbarManager` interface.

### 17.9 Command Line Interface (CLI)
- **Behavior**: The application must parse incoming command-line arguments to support being launched from terminal environments (e.g., `Files.exe C:\` to open a specific path, or flags for specific actions).

### 17.8 Status Center
- **Behavior**: Rather than intrusive modal popups, background tasks (file copying, zipping) and their respective errors/conflict resolutions must be aggregated into a non-blocking "Status Center" flyout accessible from the main toolbar.

### 17.9 Compact Overlay (Picture-in-Picture)
- **Behavior**: Provide an "Always on top" Compact Overlay mode (`WindowEx`) allowing users to pin a minimalistic version of the file explorer or a media preview to the corner of their screen.

### 17.10 App Update Mechanisms
- **Behavior**: The application handles background update checking using abstract update services (`AppUpdateStoreService`, `AppUpdateSideloadService`) to notify the user of new releases depending on their installation source.

---

## 18. Accessibility & UX Details to Match

- **Tooltip on all buttons**: label + hotkey string, e.g. "Navigate Back (Alt+Left)".
- **Keyboard navigation**: Tab moves between sidebar / file list / preview pane.
- **AutomationProperties** equivalent: unique accessible names on all interactive elements.
- **High contrast**: detect system high-contrast mode; disable transparency effects.
- **Drag hover-to-open**: hovering dragged item over a folder for 1300ms opens it.
- **Tab Interactions**:
  - **Cross-Window Drag**: Dragging a tab outside the window spawns a new process instance; dragging onto another window's tab strip merges it.
  - **Hover-to-Select**: When dragging a file over a tab, select that tab automatically after a 500ms delay (`HoverToOpenTimespan`).
  - **Scroll-to-Switch**: Scrolling the mouse wheel while hovering over the tab strip cycles through active tabs.
  - **New Tab Drag Target**: Dragging a folder onto the `+` button opens that path in a new tab.
- **Select on hover mode**: single-click selects without navigating (configurable).
- **Double-click opens**: directories navigate, files launch with default app.
- **Dynamic Fonts**: Users can specify a custom system font family via `AppThemeFontFamily`. The UI must re-render all text elements using the selected font without requiring a restart.
- **Context Menu Warmup**: Implement a background "warmup" task (`WarmUpQueryContextMenuAsync`) that initializes Shell COM objects and caches context menu schemas to ensure instantaneous right-click response.
- **Right-click drag**: shows copy/move/create-shortcut choice menu on drop.
- **Rubber-band selection**: left-button drag on empty space draws selection rect.
- **Item count in tab**: tab tooltip shows item count of open folder.

---

## 19. Embedded Assets & Resources

Files-community ships with various binary and JSON assets that must be directly ported into the Rust build. Store these inside a `resources/` or `assets/` folder and embed them via `include_bytes!` or `include_str!`.

### 19.1 Empty State SVGs
- **Sidebar**: `Files-community/src/Files.App/Assets/Sidebar/EmptySidebar_100_ThemeDark.svg` (and corresponding Light/200/48 scale variants).
- **Shelf**: `Files-community/src/Files.App/Assets/Shelf/EmptyShelf_100_ThemeDark.svg` (and corresponding Light/200/48 scale variants).

### 19.2 Platform / Distro SVGs
- **WSL Icons**: Map detected WSL distributions to `Files-community/src/Files.App/Assets/WSL/`.
  - Includes: `alpine.svg`, `debian.svg`, `kali.svg`, `opensuse.svg`, `ubuntu.svg`, and a fallback `generic.svg`.

### 19.3 Custom Feature Icons
- **Archives**: `Files-community/src/Files.App/Assets/Archives/ExtensionIcon.png` (use as the file-type icon for compressed formats).
- **Fluent Icons**: `Files-community/src/Files.App/Assets/FluentIcons/` (CloudDrive, FileTags, Home, Star).
- **Folder Overrides**: `FolderIcon.svg` and `FolderIcon2.svg`.

### 19.4 Data-Driven Property Definitions
- **Info Pane Mapping**: `Files-community/src/Files.App/Assets/Resources/PropertiesInformation.json`
  - This JSON file defines the exact layout, System Property Keys (PKEYs), and localization string keys for the right-side Info Pane. The UI should dynamically generate its metadata sections by parsing this file rather than hardcoding PKEY lookups.
- **Preview Pane Mapping**: `Files-community/src/Files.App/Assets/Resources/PreviewPanePropertiesInformation.json`
  - Similar definition file used exclusively for the preview pane metadata block.

### 19.5 Registry Templates
- **Shell Replacement Keys**: `Files-community/src/Files.App/Assets/FilesOpenDialog/SetFilesAsDefault.reg` and `UnsetFilesAsDefault.reg`. These are used by the elevated Launcher to overwrite the native `Folder\shell\open\command` keys.
---

## 20. Terminal Integration & Detection
The application provides integrated terminal launching with auto-detection of installed shells.

### 20.1 Terminal Detection Logic
The `TerminalService` queries the following registry keys and paths to identify available terminals:
- **Windows Terminal**: `HKCU\Software\Microsoft\Windows\CurrentVersion\PackagedAppInstaller\Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe` or check `%LocalAppData%\Microsoft\WindowsApps\wt.exe`.
- **Fluent Terminal**: Check `%LocalAppData%\Microsoft\WindowsApps\flute.exe`.
- **Git Bash**: `HKLM\SOFTWARE\GitForWindows` (InstallPath) or `%ProgramFiles%\Git\bin\sh.exe`.
- **PowerShell (Modern)**: `HKLM\SOFTWARE\Microsoft\PowerShell\3\PowerShellEngine` (ApplicationBase) or `%ProgramFiles%\PowerShell\7\pwsh.exe`.
- **WSL**: Enumerate distributions via `wsl.exe --list --quiet` and generate "Open in [Distro]" verbs.

### 20.2 Default Terminal
- Users can select a "Default Terminal" in Settings. 
- The `OpenTerminalAction` uses the default, while a submenu (`OpenTerminalSubMenu`) allows picking from the detected list.
- All terminal launches are invoked with the current working directory as the startup path.

---

## 21. Keyboard Remapping & Shortcut Infrastructure
To support full user customization, the application uses a dynamic `ShortcutManager`.

### 21.1 Shortcut Configuration
- **Storage**: Keybindings are stored in `shortcuts.json`.
- **Mapping**: Actions are referenced by their unique ID (e.g., `files.copy`).
- **Conflict Handling**: If two actions share a shortcut, the most specific one (e.g., a file-list specific shortcut) takes precedence over a global one.
- **UI Interaction**: The Settings -> Actions page allows users to record new key combinations. The `ShortcutRecorder` component listens for raw `KeyDown` events and captures the `Key + Modifiers` combination.

### 21.2 Default Terminal
The application provides a "Terminal Detection Service" that scans the registry and common paths for:
1. **Windows Terminal** (`wt.exe`)
2. **Git Bash** (`sh.exe`)
3. **PowerShell Core** (`pwsh.exe`)
4. **Command Prompt** (`cmd.exe`)
5. **WSL** (invoked via `wsl.exe ~`)

---
