//! Перегенерация объекта.
//!
//! `R` — полная перегенерация с новым случайным seed, `Shift + R` — повтор
//! с тем же глобальным seed. Генератор подписывается на сообщение
//! [`RegenerationRequest`] и сам решает, что именно пересоздавать.

use bevy::prelude::*;

/// Запрос перегенерации объекта (потоковое сообщение для `MessageReader`).
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RegenerationRequest {
    /// Если `true` — сохранить текущий глобальный seed (повтор того же вида),
    /// если `false` — взять новый случайный.
    pub same_seed: bool,
}

impl RegenerationRequest {
    /// Полностью новая генерация (новый seed).
    pub fn new_random() -> Self {
        Self { same_seed: false }
    }

    /// Повтор генерации с тем же seed.
    pub fn same() -> Self {
        Self { same_seed: true }
    }
}
