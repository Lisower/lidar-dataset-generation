//! Приложение-генератор ландшафта: окно Bevy с визуализацией и управлением.
//!
//! Собран из общих плагинов:
//! - [`GenerationControlsPlugin`] — горячие клавиши (`V`, `R`, `Shift + R`);
//! - [`CameraPlugin`](bevy_utils::cameras::CameraPlugin) — орбитальная и
//!   свободная камеры (`Tab`);
//! - [`LandscapePlugin`](landscapes_generator::LandscapePlugin) — сцена,
//!   панель информации и переключение пресетов (`P`).
//!
//! Для headless-использования этой же генерации обратитесь к крейту как к
//! библиотеке ([`landscapes_generator::generation`]).

use bevy::prelude::*;
use bevy_utils::cameras::CameraPlugin;
use generation_core::GenerationControlsPlugin;
use landscapes_generator::LandscapePlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            GenerationControlsPlugin::default(),
            CameraPlugin::new(Vec3::new(0.0, 4.0, 0.0), 90.0),
            LandscapePlugin::default(),
        ))
        .run();
}
