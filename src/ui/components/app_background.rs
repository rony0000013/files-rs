use crate::ui::constants::UiConstants;
use crate::ui::views::main_view::BackgroundFit;
use crate::ui::views::main_view::FolderBackground;
use gpui::*;

pub struct AppBackground;

impl AppBackground {
    pub fn build(
        folder_bg: &FolderBackground,
        _mica_enabled: bool,
        _mica_alt: bool,
        children: Vec<AnyElement>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(UiConstants::BG_MAIN_RGB))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .h_full()
                    .relative()
                    .children(children),
            )
    }
}
