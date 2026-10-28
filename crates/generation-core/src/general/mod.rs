//! Общий функционал генераторов.
//!
//! Режимы отображения объекта и перегенерация — общие для всех генераторов:
//! и для визуализации, и для headless-сценариев.

pub mod controls;
pub mod regeneration;
pub mod ui_lock;
pub mod view_modes;

pub use controls::GenerationControlsPlugin;
pub use regeneration::RegenerationRequest;
pub use ui_lock::UiModalOpen;
pub use view_modes::{ViewMode, ViewModeToggle};
