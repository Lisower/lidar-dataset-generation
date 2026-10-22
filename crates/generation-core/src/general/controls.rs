//! Горячие клавиши общего управления генерацией: смена режима отображения,
//! перегенерация. Единый плагин подходит всем генераторам.

use bevy::prelude::*;

use super::regeneration::RegenerationRequest;
use super::view_modes::{ViewMode, ViewModeToggle};
use crate::seeds::{GlobalSeed, Seed};

/// Плагин общих горячих клавиш генератора.
///
/// - `V` — сменить режим отображения ([`ViewMode`]);
/// - `R` — запросить полную перегенерацию (новый случайный seed в
///   [`GlobalSeed`]);
/// - `Shift + R` — перегенерировать с тем же глобальным seed.
///
/// Приложение-генератор слушает [`RegenerationRequest`] (или просто читает
/// [`GlobalSeed`]) и пересоздаёт свои объекты; визуализации реагируют на
/// смену [`ViewMode`].
pub struct GenerationControlsPlugin {
    /// Начальное значение глобального seed.
    pub initial_seed: Seed,
}

impl Default for GenerationControlsPlugin {
    fn default() -> Self {
        Self {
            initial_seed: Seed::default(),
        }
    }
}

impl Plugin for GenerationControlsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GlobalSeed(self.initial_seed))
            .insert_resource(ViewMode::default())
            .add_message::<RegenerationRequest>()
            .add_message::<ViewModeToggle>()
            .add_systems(Update, (handle_regeneration_key, handle_view_mode_key));
    }
}

/// `R` / `Shift + R`.
fn handle_regeneration_key(
    keys: Res<ButtonInput<KeyCode>>,
    mut writer: MessageWriter<RegenerationRequest>,
) {
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        // Тот же seed — повтор того же вида.
        writer.write(RegenerationRequest::same());
    } else {
        // Новый случайный seed.
        writer.write(RegenerationRequest::new_random());
    }
}

/// `V` — смена режима отображения.
fn handle_view_mode_key(
    keys: Res<ButtonInput<KeyCode>>,
    mut view_mode: ResMut<ViewMode>,
    mut writer: MessageWriter<ViewModeToggle>,
) {
    if keys.just_pressed(KeyCode::KeyV) {
        *view_mode = view_mode.toggled();
        writer.write(ViewModeToggle);
    }
}
