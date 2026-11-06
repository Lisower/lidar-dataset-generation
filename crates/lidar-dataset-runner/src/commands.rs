//! Команды Tauri: тонкие адаптеры между интерфейсом и `dataset/`.
//!
//! Здесь только разбор аргументов, проверка состояния, запуск тяжёлой работы
//! в blocking-пуле и эмиссия событий. Генерация CPU-тяжёлая и синхронная,
//! поэтому не выполняется в UI-потоке: иначе окно замирает (`DESIGN.md` §5.2,
//! анти-цель «UI замирает на время генерации»).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use generation_core::PointClass;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::dataset::catalog::{Catalog, load_catalog};
use crate::dataset::job::{BatchOptions, run_batch};
use crate::dataset::recipe::{Recipe, validate};
use crate::state::{AppState, JobHandle};

/// Событие прогресса.
pub const EVENT_PROGRESS: &str = "generation://progress";
/// Событие завершения запуска.
pub const EVENT_FINISHED: &str = "generation://finished";
/// Событие фатальной ошибки запуска.
pub const EVENT_FAILED: &str = "generation://failed";

/// Каталог пресетов плюс найденный путь к нему.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogInfo {
    /// Путь к каталогу, если он найден.
    pub presets_root: Option<String>,
    /// Сам каталог.
    pub catalog: Catalog,
}

/// Класс точек для легенды интерфейса.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassDto {
    /// Числовой код класса (контракт форматов экспорта).
    pub code: u8,
    /// Английская подпись из `PointClass::label`.
    pub name: String,
    /// Русское название класса.
    pub label: String,
    /// Цвет в hex, посчитанный из `PointClass::color`.
    pub color: String,
}

/// Отдать каталог пресетов и найденный путь.
#[tauri::command]
pub fn catalog(state: State<'_, AppState>) -> Result<CatalogInfo, String> {
    let root = state.paths.presets_root.clone();
    let catalog = match &root {
        Some(path) => load_catalog(path),
        None => Catalog::default(),
    };
    Ok(CatalogInfo {
        presets_root: root.map(|path| path.to_string_lossy().into_owned()),
        catalog,
    })
}

/// Отдать палитру классов точек.
#[tauri::command]
pub fn point_classes() -> Vec<ClassDto> {
    [
        PointClass::Ground,
        PointClass::Building,
        PointClass::Vegetation,
        PointClass::Water,
        PointClass::LowNoise,
        PointClass::Other,
    ]
    .into_iter()
    .map(|class| ClassDto {
        code: class.code(),
        name: class.label().to_string(),
        label: russian_label(class).to_string(),
        color: hex_color(class),
    })
    .collect()
}

/// Каталог вывода по умолчанию.
#[tauri::command]
pub fn default_output_dir(app: AppHandle) -> Result<String, String> {
    let documents = app.path().document_dir().ok();
    Ok(AppState::default_output_dir(documents.as_deref())
        .to_string_lossy()
        .into_owned())
}

/// Запустить генерацию и сразу вернуть идентификатор задачи.
///
/// Рецепт валидируется синхронно, чтобы ошибки полей вернулись в форму до
/// старта. Сама генерация уходит в blocking-пул.
#[tauri::command]
pub async fn start_generation(
    app: AppHandle,
    recipe: Recipe,
    output_dir: String,
) -> Result<String, String> {
    let state = app.state::<AppState>();
    let root = state
        .paths
        .presets_root
        .clone()
        .ok_or_else(|| "каталог пресетов не найден: генерация недоступна".to_string())?;
    let catalog = load_catalog(&root);

    let issues = validate(&recipe, &catalog);
    if !issues.is_empty() {
        return Err(serde_json::to_string(&issues).unwrap_or_else(|_| "[]".to_string()));
    }
    if state.current_job().is_some() {
        return Err("генерация уже идёт: дождитесь завершения или отмените её".to_string());
    }

    let output = PathBuf::from(&output_dir);
    std::fs::create_dir_all(&output)
        .map_err(|error| format!("не удалось создать каталог вывода: {error}"))?;

    let job_id = job_id(&recipe);
    let cancel = Arc::new(AtomicBool::new(false));
    state.set_job(Some(JobHandle {
        id: job_id.clone(),
        cancel: cancel.clone(),
    }));

    let handle = app.clone();
    let options = BatchOptions {
        output_dir: output,
        job_id: job_id.clone(),
        cancel,
    };
    tauri::async_runtime::spawn_blocking(move || {
        let mut on_progress = |event: &crate::dataset::stats::ProgressEvent| {
            let _ = handle.emit(EVENT_PROGRESS, event.clone());
        };
        let report = run_batch(&catalog, &recipe, &options, &mut on_progress);
        let state = handle.state::<AppState>();
        state.set_job(None);
        let _ = handle.emit(EVENT_FINISHED, report);
    });

    Ok(job_id)
}

/// Отменить запуск `job_id`; `false` — такой задачи нет.
#[tauri::command]
pub fn cancel_generation(state: State<'_, AppState>, job_id: String) -> bool {
    match state.cancel_flag(&job_id) {
        Some(flag) => {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
            true
        }
        None => false,
    }
}

/// Идентификатор запуска из рецепта и времени старта.
fn job_id(recipe: &Recipe) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("{}-{nanos}", recipe.seed)
}

/// Русское название класса точек.
fn russian_label(class: PointClass) -> &'static str {
    match class {
        PointClass::Ground => "грунт",
        PointClass::Building => "здания",
        PointClass::Vegetation => "растительность",
        PointClass::Water => "вода",
        PointClass::LowNoise => "шум",
        PointClass::Other => "прочее",
    }
}

/// Цвет класса в hex без альфы.
fn hex_color(class: PointClass) -> String {
    let color = class.color().to_srgba();
    format!(
        "#{:02x}{:02x}{:02x}",
        (color.red * 255.0).round() as u8,
        (color.green * 255.0).round() as u8,
        (color.blue * 255.0).round() as u8
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_classes_expose_codes_colors_and_russian_names() {
        let classes = point_classes();
        assert_eq!(classes.len(), 6);
        let ground = classes.iter().find(|c| c.code == 2).unwrap();
        assert_eq!(ground.name, "ground");
        assert_eq!(ground.label, "грунт");
        assert!(ground.color.starts_with('#') && ground.color.len() == 7);
        let codes: Vec<u8> = classes.iter().map(|c| c.code).collect();
        assert_eq!(codes, vec![2, 6, 5, 9, 7, 1]);
    }
}
