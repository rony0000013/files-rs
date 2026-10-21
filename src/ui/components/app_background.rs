use crate::ui::constants::UiConstants;
use crate::ui::views::main_view::{BackgroundFit, FolderBackground};
use gpui::*;

pub struct AppBackground;

impl AppBackground {
    pub fn build(
        folder_bg: &FolderBackground,
        mica_enabled: bool,
        mica_alt: bool,
        mica_geo: Option<crate::wallpaper_mica::MicaGeo>,
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
        // so we render those exact pixels ourselves.) The blur is a
        // full-virtual-screen image positioned per-frame from the live
        // window bounds — moving the window slides it instantly, with zero
        // regen cost (regen happens only on wallpaper change).
        if let Some(geo) = mica_geo {
            root = root.child(
                img(geo.path)
                    .absolute()
                    .left(px(geo.left))
                    .top(px(geo.top))
                    .w(px(geo.w))
                    .h(px(geo.h)),
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
