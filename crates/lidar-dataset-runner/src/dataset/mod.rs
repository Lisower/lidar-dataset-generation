//! Чистая логика генерации датасетов: рецепт, каталог пресетов, композиция
//! сцены, сборка облака, статистика, экспорт и батч-цикл.
//!
//! Модуль не знает про Tauri и `AppHandle`: прогресс отдаётся колбэком, отмена
//! — флагом `AtomicBool`. Поэтому всё здесь тестируется обычным `cargo test`.

pub mod catalog;
pub mod export;
pub mod job;
pub mod merge;
pub mod recipe;
pub mod scene;
pub mod stats;

#[cfg(test)]
pub(crate) mod test_support;
