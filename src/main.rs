use files_rs::app::{AppState, AppWindow};
use files_rs::config::config::AppConfig;
use files_rs::config::settings_model::SettingsModel;
use files_rs::core::database::Database;
use files_rs::core::path_util::FilesPath;
use files_rs::ui::views::main_view::FileItems;
use gpui::AppContext as _;
use gpui::*;
use std::sync::Arc;

fn main() {
    files_rs::core::logger::Logger::init();

    let db_path = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("files-rs")
        .join("tags.db");
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let db = Arc::new(
        Database::new(db_path).unwrap_or_else(|_| {
            Database::new(std::path::PathBuf::from(":memory:"))
                .expect("Failed to initialize fallback in-memory database")
        }),
    );
    let progress = Arc::new(files_rs::core::progress::ProgressEngine::new());
    let root = dirs::home_dir().unwrap_or_else(|| {
        #[cfg(windows)]
        {
            std::path::PathBuf::from("C:\\")
        }
        #[cfg(not(windows))]
        {
            std::path::PathBuf::from("/")
        }
    });

    Application::new().run(move |cx| {
        let config = AppConfig::load();
        let initial_view_mode = config.view.last_view_mode.clone();
        let mica_enabled = config.appearance.mica_enabled;
        let mica_alt = config.appearance.mica_alt;
        let transparent_window = config.appearance.transparent_window;
        let settings = cx.new(|_cx| SettingsModel::new(config));
        let nav = cx.new(|_cx| {
            files_rs::core::navigation_manager::NavigationState::new(
                FilesPath::new(&root),
                initial_view_mode,
            )
        });
        let loc = cx.new(|_cx| files_rs::localization::localization::LocalizationManager::new());
        let slot = cx.reserve_entity();
        let file_items = cx.insert_entity(slot, |_| FileItems::default());
        let state = AppState::new(
            FilesPath::new(&root),
            file_items,
            settings,
            loc,
            nav,
            progress.clone(),
            db.clone(),
            cx,
        );

        // gpui 0.2.2 only exposes Opaque/Transparent/Blurred (no native Mica
        // variants). Use Opaque (ACCENT_DISABLED: no legacy composition
        // policy): both Transparent (TRANSPARENTGRADIENT) and Blurred
        // (ACRYLICBLURBEHIND) install legacy accent policies that fight
        // window_vibrancy's DWMWA_SYSTEMBACKDROP_TYPE and composite opaque.
        // The actual Mica comes from apply_mica below; translucency comes
        // from the RGBA fills in the scene, not the window policy.
        let window_background = if transparent_window {
            WindowBackgroundAppearance::Transparent
        } else {
            WindowBackgroundAppearance::Opaque
        };

        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Maximized(Bounds::new(
                point(px(0.0), px(0.0)),
                size(px(1280.0), px(800.0)),
            ))),
            titlebar: None,
            window_background,
            ..Default::default()
        };

        cx.open_window(opts, move |window, cx| {
            #[cfg(windows)]
            if mica_enabled {
                if mica_alt {
                    if let Err(e) = window_vibrancy::apply_tabbed(&*window, Some(true)) {
                        tracing::error!("apply_tabbed (MicaAlt) failed: {:?}", e);
                    } else {
                        tracing::info!("apply_tabbed (MicaAlt) applied");
                    }
                } else if let Err(e) = window_vibrancy::apply_mica(&*window, Some(true)) {
                    tracing::error!("apply_mica failed: {:?}", e);
                } else {
                    tracing::info!("apply_mica applied");
                }
                // In-app Mica: last run's blurred wallpaper shows instantly;
                // a background thread regenerates it if the wallpaper changed.
                files_rs::wallpaper_mica::prime();
                std::thread::spawn(files_rs::wallpaper_mica::ensure_async);
            }
            let rx = progress.subscribe();
            cx.new(|cx| AppWindow::new(state, rx, window, cx))
        })
        .expect("Failed to open window");
    });
}
