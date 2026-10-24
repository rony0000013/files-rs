/// UI Constants for cohesive design and easy maintenance.
/// Aligned with Files Community (WinUI 3) design specifications.

pub struct UiConstants;

impl UiConstants {
    // ─────────────────────────────────────────────────────────────────────────────
    // COLORS — Backgrounds (Layered for Windows 11 Mica / Mica Alt)
    // ─────────────────────────────────────────────────────────────────────────────

    /// Mica recipe (dark): DWM draws blurred-wallpaper Mica behind the window;
    /// every full-area fill below is translucent so it shows through.
    /// Root + chrome ~65% (Mica clearly visible), content ~85% (readable),
    /// menus ~95% (solid for contrast).
    pub const BG_MICA_ROOT_RGBA: u32 = 0x1F1F1FA6;
    pub const BG_MICA_CHROME_RGBA: u32 = 0x232323A6;
    pub const BG_MICA_CONTENT_RGBA: u32 = 0x1C1C1CD9;
    /// Main window background (Translucent layer over Mica, or solid fallback)
    pub const BG_MAIN_RGBA: u32 = 0x1C1C1CFF;
    pub const BG_MAIN_MICA_ALT_RGBA: u32 = 0x141414CC;
    pub const BG_MAIN_RGB: u32 = 0x1C1C1C;

    /// Sidebar background (Translucent so DWM Mica shows through)
    pub const BG_SIDEBAR_RGBA: u32 = 0x202020FF;
    pub const BG_SIDEBAR_RGB: u32 = 0x202020;

    /// Dialog background (Acrylic / Elevated Flyout surface)
    pub const BG_DIALOG_RGB: u32 = 0x2B2B2B;
    pub const BG_DIALOG_RGBA: u32 = 0x2B2B2BF2;

    /// Tab Bar background (Transparent over Mica titlebar)
    pub const BG_TAB_BAR_RGBA: u32 = 0x1A1A1A00;

    /// Navigation / Breadcrumb Bar background
    pub const BG_NAV_BAR_RGBA: u32 = 0x1C1C1CE6;
    pub const BG_NAV_BAR_RGB: u32 = 0x1C1C1C;

    /// Toolbar background (WinUI 3 LayerFillColorDefault card layer)
    pub const BG_TOOLBAR_RGBA: u32 = 0x2C2C2CFF;
    pub const BG_TOOLBAR_RGB: u32 = 0x2C2C2C;

    /// Status bar background (Translucent footer over Mica)
    pub const BG_STATUS_BAR_RGBA: u32 = 0x202020CC;

    /// Inactive pane dimming
    pub const BG_INACTIVE_PANE_RGBA: u32 = 0x00000018;

    /// Item hover: SubtleFillColorSecondary (~6% white)
    pub const BG_ITEM_HOVER_RGBA: u32 = 0xFFFFFF0F;

    /// Item selected: SubtleFillColorTertiary (~9.4% white)
    pub const BG_ITEM_SELECTED_RGBA: u32 = 0xFFFFFF18;

    /// Item selected+hover
    pub const BG_ITEM_SELECTED_HOVER_RGBA: u32 = 0xFFFFFF25;

    /// Badge background (~20% white)
    pub const BG_BADGE_RGBA: u32 = 0xFFFFFF33;

    /// Transparent fallback
    pub const BG_TRANSPARENT_RGBA: u32 = 0x00000000;

    // ─────────────────────────────────────────────────────────────────────────────
    // COLORS — Accent & State
    // ─────────────────────────────────────────────────────────────────────────────

    /// Files-community blue accent (#0078D4 / #005FB8), fully opaque
    pub const ACCENT_RGBA: u32 = 0x0078D4FF;
    pub const ACCENT_COLOR_RGB: u32 = 0x0078D4;
    pub const DANGER_RGBA: u32 = 0xFF99A4FF;
    pub const SYSTEM_FILL_COLOR_CRITICAL_RGB: u32 = 0xC42B1C;

    // ─────────────────────────────────────────────────────────────────────────────
    // COLORS — Text (WinUI 3 Semantic)
    // ─────────────────────────────────────────────────────────────────────────────

    /// ~89% white (WinUI3 TextFillColorPrimary)
    pub const TEXT_PRIMARY_RGBA: u32 = 0xFFFFFFE4;
    /// ~62% white (WinUI3 TextFillColorSecondary)
    pub const TEXT_SECONDARY_RGBA: u32 = 0xFFFFFF9D;
    /// ~36% white (WinUI3 TextFillColorTertiary/Disabled)
    pub const TEXT_MUTED_RGBA: u32 = 0xFFFFFF5C;
    pub const TEXT_PRIMARY_RGB: u32 = 0xFFFFFF;
    pub const TEXT_SECONDARY_RGB: u32 = 0x9D9D9D;
    pub const TEXT_MUTED_RGB: u32 = 0x5C5C5C;

    // ─────────────────────────────────────────────────────────────────────────────
    // Borders & Dividers (WinUI 3 Fluent 1px Strokes)
    // ─────────────────────────────────────────────────────────────────────────────

    /// Subtle 1px card border stroke (#FFFFFF0F on dark mode)
    pub const CARD_STROKE_RGBA: u32 = 0xFFFFFF0F;
    /// Flyout/menu surface border stroke (slightly stronger than card stroke)
    pub const SURFACE_STROKE_FLYOUT_RGBA: u32 = 0xFFFFFF1F;
    /// ~8% white divider (DividerStrokeColorDefault)
    pub const DIVIDER_RGBA: u32 = 0xFFFFFF15;
    pub const DIVIDER_RGB: u32 = 0x333333;
    /// ~7% white control stroke (ControlStrokeColorDefault)
    pub const STROKE_RGBA: u32 = 0xFFFFFF12;
    pub const STROKE_RGB: u32 = 0x404040;

    // ─────────────────────────────────────────────────────────────────────────────
    // LAYOUT — Dimensions
    // ─────────────────────────────────────────────────────────────────────────────

    pub const SIDEBAR_WIDTH_RAW: f64 = 256.0;
    pub const SIDEBAR_WIDTH_COLLAPSED_RAW: f64 = 48.0;
    pub const SIDEBAR_WIDTH_MIN_RAW: f64 = 180.0;
    pub const SIDEBAR_WIDTH_MAX_RAW: f64 = 500.0;
    pub const ITEM_HEIGHT_RAW: f64 = 32.0;
    pub const SECTION_HEADER_HEIGHT_RAW: f64 = 28.0;
    pub const TAB_BAR_HEIGHT: f64 = 40.0;
    pub const TOOLBAR_HEIGHT: f64 = 40.0;
    pub const NAV_BAR_HEIGHT: f64 = 48.0;
    pub const STATUS_BAR_HEIGHT_RAW: f64 = 32.0;
    pub const GRID_ITEM_HEIGHT: f64 = 130.0;
    pub const GRID_GAP_RAW: f64 = 8.0;
    pub const GRID_ITEM_SIZE: f64 = 120.0;
    pub const GRID_ICON_SIZE: f64 = 72.0;
    pub const SPLITTER_WIDTH_RAW: f64 = 4.0;

    // ─────────────────────────────────────────────────────────────────────────────
    // TYPOGRAPHY & SPACING
    // ─────────────────────────────────────────────────────────────────────────────

    pub const FONT_MAIN: &'static str = "Segoe UI Variable Text";
    pub const FONT_FAMILY_DEFAULT: &'static str = "Segoe UI Variable Text";
    pub const FONT_ICONS: &'static str = "Segoe Fluent Icons";
    pub const FONT_MDL2: &'static str = "Segoe Fluent Icons";

    pub const ITEM_FONT_SIZE_RAW: f64 = 13.0;
    pub const SECTION_HEADER_FONT_SIZE_RAW: f64 = 11.0;
    pub const TEXT_SIZE_STATUS_BAR: f64 = 12.0;
    pub const PADDING_BASE: f64 = 12.0;
    pub const PADDING_LARGE_RAW: f64 = 16.0;
    pub const PADDING_MEDIUM_RAW: f64 = 8.0;
    pub const PADDING_SMALL_RAW: f64 = 4.0;
    pub const BORDER_RADIUS_SMALL_RAW: f64 = 4.0;

    pub const ICON_SIZE_RAW: f64 = 24.0;
    pub const ICON_SIZE_MEDIUM: f64 = 16.0;
    pub const ICON_SIZE_SIDEBAR: f64 = 16.0;
    pub const ICON_SIZE_SMALL: f64 = 16.0;
    pub const ICON_SIZE_LARGE: f64 = 64.0;
    pub const ICON_SIZE_FILE_LIST: f64 = 24.0;
    pub const NAV_BTN_WIDTH: f64 = 36.0;
    pub const NAV_BTN_HEIGHT: f64 = 32.0;

    pub const SELECTED_BORDER_WIDTH: f64 = 3.0;
}
