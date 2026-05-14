use std::sync::Mutex;
use taffy::prelude::*;

/// LayoutManager handles the calculation of UI elements' dimensions and positions
/// using Taffy's Flexbox implementation.
#[allow(dead_code)]
pub struct LayoutManager {
    taffy: Mutex<TaffyTree>,
}

#[allow(dead_code)]
impl LayoutManager {
    pub fn new() -> Self {
        Self {
            taffy: Mutex::new(TaffyTree::new()),
        }
    }

    /// Calculate a basic layout for the main window structure
    pub fn calculate_main_layout(&self, window_width: f32, window_height: f32) -> LayoutResult {
        let mut taffy = self.taffy.lock().unwrap();

        // Taffy 0.10 API: use new_leaf for root and children
        // Note: TaffyTree doesn't have a specific "root" method in 0.10, we just use a leaf as root.
        let root = taffy
            .new_leaf(Style {
                size: Size {
                    width: Dimension::length(window_width),
                    height: Dimension::length(window_height),
                },
                flex_direction: FlexDirection::Row,
                ..Default::default()
            })
            .unwrap();

        let sidebar = taffy
            .new_leaf(Style {
                size: Size {
                    width: Dimension::length(250.0),
                    height: Dimension::length(window_height),
                },
                ..Default::default()
            })
            .unwrap();
        let _ = taffy.add_child(root, sidebar);

        let content = taffy
            .new_leaf(Style {
                size: Size {
                    width: Dimension::auto(),
                    height: Dimension::length(window_height),
                },
                flex_direction: FlexDirection::Column,
                ..Default::default()
            })
            .unwrap();
        let _ = taffy.add_child(root, content);

        taffy
            .compute_layout(
                root,
                Size {
                    width: AvailableSpace::Definite(window_width),
                    height: AvailableSpace::Definite(window_height),
                },
            )
            .unwrap();

        LayoutResult {
            sidebar_width: taffy.layout(sidebar).map(|l| l.size.width).unwrap_or(250.0),
            content_width: taffy
                .layout(content)
                .map(|l| l.size.width)
                .unwrap_or(window_width - 250.0),
        }
    }
}

pub struct LayoutResult {
    pub sidebar_width: f32,
    pub content_width: f32,
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self::new()
    }
}
