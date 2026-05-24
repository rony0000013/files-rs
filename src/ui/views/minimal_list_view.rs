//! Minimal List View for Debugging
//!
//! This provides a stripped-down, reliable list implementation to isolate the visibility issue.

use crate::domain::metadata::FileItem;
use crate::ui::views::main_view::{FileItems, MainViewState};
use gpui::*;

impl MainViewState {
    /// A minimal, non-virtualized list view to test if rendering works at all.
    pub fn render_minimal_list_view(
        &self,
        _file_items: Entity<FileItems>,
        _view: Entity<crate::app::AppWindow>,
    ) -> AnyElement {
        let visible = self.visible_items();
        
        div()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(0xffffff))
            .children(visible.iter().map(|&idx| {
                let item = &self.items[idx];
                div()
                    .h(px(30.0))
                    .px(px(10.0))
                    .border_b(px(1.0))
                    .border_color(rgb(0xcccccc))
                    .child(format!("File: {}", item.name))
                    .into_any_element()
            }))
            .into_any_element()
    }
}
