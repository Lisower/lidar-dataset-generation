//! Приложение-генератор зданий: окно Bevy с визуализацией и управлением.
//!
//! Собран из общих плагинов:
//! - [`GenerationControlsPlugin`] — горячие клавиши (`V`, `R`, `Shift + R`);
//! - [`CameraPlugin`](bevy_utils::cameras::CameraPlugin) — орбитальная и
//!   свободная камеры (`Tab`);
//! - [`BuildingPlugin`](buildings_generator::BuildingPlugin) — сцена и объекты.
//!
//! Для headless-использования этой же генерации обратитесь к крейту как к
//! библиотеке ([`buildings_generator::generation`]).

use bevy::prelude::*;
use bevy_utils::cameras::CameraPlugin;
use buildings_generator::BuildingPlugin;
use generation_core::GenerationControlsPlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            GenerationControlsPlugin::default(),
            CameraPlugin::new(Vec3::new(0.0, 3.0, 0.0), 60.0),
            BuildingPlugin::default(),
        ))
        .run();
}
