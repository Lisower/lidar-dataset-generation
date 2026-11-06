//! Десктопный раннер датасетов LiDAR.
//!
//! Бэкенд Tauri-приложения: собирает сцены из существующих генераторов
//! (ландшафты, здания, растения) и экспортирует каждую сцену одним файлом
//! облака точек. Вся чистая логика живёт в [`dataset`] и не зависит от
//! `AppHandle`, поэтому тестируется обычным `cargo test`.

pub mod commands;
pub mod dataset;
pub mod error;
pub mod state;

use tauri::Manager;

use crate::state::{AppPaths, AppState};

/// Запустить десктопное приложение.
///
/// `generate_context!` держится только здесь, чтобы юнит-тесты чистой логики
/// не зависели от наличия собранного фронтенда (`ui/dist`).
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let resource_dir = app.path().resource_dir().ok();
            let env_override = std::env::var_os("LIDAR_PRESETS_ROOT").map(Into::into);
            let paths = AppPaths::resolve(resource_dir, env_override);
            match &paths.presets_root {
                Some(root) => eprintln!("presets_root: {}", root.display()),
                None => eprintln!(
                    "presets_root не найден: генерация будет недоступна до появления каталога presets/"
                ),
            }
            app.manage(AppState::new(paths));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::catalog,
            commands::point_classes,
            commands::default_output_dir,
            commands::start_generation,
            commands::cancel_generation,
        ])
        .run(tauri::generate_context!())
        .expect("не удалось запустить приложение");
}
