//! Приложение-генератор растений: окно Bevy с визуализацией и управлением.
//!
//! Собран из общих плагинов:
//! - [`GenerationControlsPlugin`] — горячие клавиши (`V`, `R`, `Shift + R`);
//! - [`CameraPlugin`](bevy_utils::cameras::CameraPlugin) — орбитальная и
//!   свободная камеры (`Tab`);
//! - [`PlantsPlugin`](plants_generator::PlantsPlugin) — сцена и объекты,
//!   а также клавиши пресета (`P`) и режима генерации (`M`).
//!
//! Для headless-использования этой же генерации обратитесь к крейту как к
//! библиотеке ([`plants_generator::generation`]).

use bevy::prelude::*;
use bevy_utils::cameras::CameraPlugin;
use generation_core::GenerationControlsPlugin;
use plants_generator::PlantsPlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            GenerationControlsPlugin::default(),
            CameraPlugin::new(Vec3::new(0.0, 3.0, 0.0), 60.0),
            PlantsPlugin::default(),
        ))
        .run();
}
