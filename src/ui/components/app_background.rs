use crate::ui::constants::UiConstants;
use crate::ui::views::main_view::{BackgroundFit, FolderBackground};
use gpui::*;

pub struct AppBackground;

impl AppBackground {
    pub fn build(
        folder_bg: &FolderBackground,
        mica_enabled: bool,
        mica_alt: bool,
        children: Vec<AnyElement>,
    ) -> impl IntoElement {
        let base_bg = if mica_enabled {
            if mica_alt {
                rgba(UiConstants::BG_MAIN_MICA_ALT_RGBA)
            } else {
                rgba(UiConstants::BG_MICA_ROOT_RGBA)
            }
        } else {
            rgb(UiConstants::BG_MAIN_RGB)
        };

        let object_fit = match folder_bg.fit {
            BackgroundFit::Fill | BackgroundFit::Stretch => ObjectFit::Cover,
            BackgroundFit::Fit | BackgroundFit::Center => ObjectFit::Contain,
        };

        // Stack (bottom → top): blurred wallpaper → dark tint → optional
        // per-folder image → content. The tint over blur is the Mica recipe.
        let mut root = div().flex().flex_col().w_full().h_full().relative();

        // In-app Mica: blurred wallpaper under the translucent tint.
        // (DWM backdrop never reaches the screen through GPUI's swapchain,
        // so we render those exact pixels ourselves.)
        if let Some(blur_path) = crate::wallpaper_mica::current() {
            root = root.child(
                img(blur_path)
                    .absolute()
                    .inset_0()
                    .w_full()
                    .h_full()
                    .object_fit(ObjectFit::Cover),
            );
        }

        root = root.child(
            div()
                .absolute()
                .inset_0()
                .w_full()
                .h_full()
                .bg(base_bg),
        );

        if let Some(ref bg_path) = folder_bg.image_path {
            let opacity = folder_bg.opacity.clamp(0.0, 1.0);
            if opacity > 0.0 {
                root = root.child(
                    img(bg_path.as_path().to_path_buf())
                        .absolute()
                        .inset_0()
                        .w_full()
                        .h_full()
                        .object_fit(object_fit)
                        .opacity(opacity),
                );
            }
        }

        root.child(
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
