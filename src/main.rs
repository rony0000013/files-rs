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
    #[cfg(windows)]
    {
        files_rs::core::logger::Logger::init();
    }

    let db = Arc::new(Database::new(std::path::PathBuf::from(":memory:")).unwrap());
    let progress = Arc::new(files_rs::core::progress::ProgressEngine::new());
    let root = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("C:\\"));

    Application::new().run(move |cx| {
        let config = AppConfig::load();
        let initial_view_mode = config.view.last_view_mode.clone();
        let settings = cx.new(|cx| SettingsModel::new(config));
        let nav = cx.new(|cx| {
            files_rs::core::navigation_manager::NavigationState::new(
                FilesPath::new(&root),
                initial_view_mode,
            )
        });
        let loc = cx.new(|cx| files_rs::localization::localization::LocalizationManager::new());
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

        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Maximized(Bounds::new(
                point(px(0.0), px(0.0)),
                size(px(1280.0), px(800.0)),
            ))),
            titlebar: None,
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        };

        cx.open_window(opts, move |window, cx| {
            #[cfg(windows)]
            {
                let _ = window_vibrancy::apply_mica(&*window, Some(true));
            }
            let rx = progress.subscribe();
            cx.new(|cx| AppWindow::new(state, rx, window, cx))
        })
        .expect("Failed to open window");
    });
}
