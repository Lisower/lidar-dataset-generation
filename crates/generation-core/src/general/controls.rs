//! Горячие клавиши общего управления генерацией: смена режима отображения,
//! перегенерация. Единый плагин подходит всем генераторам.

use bevy::prelude::*;

use super::regeneration::RegenerationRequest;
use super::view_modes::{ViewMode, ViewModeToggle};
use crate::UiModalOpen;
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
        app.init_resource::<UiModalOpen>()
            .insert_resource(GlobalSeed(self.initial_seed))
            .insert_resource(ViewMode::default())
            .add_message::<RegenerationRequest>()
            .add_message::<ViewModeToggle>()
            .add_systems(Update, (handle_regeneration_key, handle_view_mode_key));
    }
}

/// `R` / `Shift + R`.
fn handle_regeneration_key(
    ui: Res<UiModalOpen>,
    keys: Res<ButtonInput<KeyCode>>,
    mut writer: MessageWriter<RegenerationRequest>,
) {
    if ui.0 || !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    if keys.pressed(KeyCode::ShiftLeft)
        || keys.pressed(KeyCode::ShiftRight)
        || keys.just_pressed(KeyCode::ShiftLeft)
        || keys.just_pressed(KeyCode::ShiftRight)
    {
        // Тот же seed — повтор того же вида.
        writer.write(RegenerationRequest::same());
    } else {
        // Новый случайный seed.
        writer.write(RegenerationRequest::new_random());
    }
}

/// `V` — смена режима отображения.
fn handle_view_mode_key(
    ui: Res<UiModalOpen>,
    keys: Res<ButtonInput<KeyCode>>,
    mut view_mode: ResMut<ViewMode>,
    mut writer: MessageWriter<ViewModeToggle>,
) {
    if ui.0 || !keys.just_pressed(KeyCode::KeyV) {
        return;
    }
    *view_mode = view_mode.toggled();
    writer.write(ViewModeToggle);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fast_shift_chord_keeps_seed_even_if_released_before_update() {
        let mut app = App::new();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::ShiftLeft);
        keys.press(KeyCode::KeyR);
        keys.release(KeyCode::KeyR);
        keys.release(KeyCode::ShiftLeft);
        app.init_resource::<UiModalOpen>()
            .insert_resource(keys)
            .add_message::<RegenerationRequest>()
            .add_systems(Update, handle_regeneration_key);
        app.update();
        let events: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<RegenerationRequest>>()
            .drain()
            .collect();
        assert_eq!(events, vec![RegenerationRequest::same()]);
    }
}
