//! Батч-цикл: сцены, прогресс, отмена, отчёт.
//!
//! Цикл не знает про Tauri: прогресс отдаётся колбэком, отмена — флагом
//! `AtomicBool`. Поэтому его поведение (включая отмену посреди записи и
//! накопление ошибок сцен) проверяется обычным тестом.
//!
//! Ошибка одной сцены не роняет запуск: сцена попадает в `errors` отчёта, а
//! цикл идёт дальше. `run.json` пишется всегда — и при отмене, и когда упали
//! все сцены, иначе каталог выглядел бы как «ничего не происходило».

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use super::catalog::Catalog;
use super::export::{write_run_report, write_scene};
use super::merge::scene_cloud;
use super::recipe::Recipe;
use super::scene::compose;
use super::stats::{ProgressEvent, RunReport, SceneError, SceneStats, Totals, scene_stats};

/// Параметры запуска батча.
pub struct BatchOptions {
    /// Каталог вывода.
    pub output_dir: PathBuf,
    /// Идентификатор запуска (для событий и отчёта).
    pub job_id: String,
    /// Флаг отмены: выставляется извне, цикл читает его перед каждой сценой.
    pub cancel: Arc<AtomicBool>,
}

/// Выполнить батч по рецепту и вернуть отчёт.
///
/// `on_progress` вызывается после каждой успешно записанной сцены; из него
/// разрешено выставлять флаг отмены (так тест проверяет прерывание).
pub fn run_batch(
    catalog: &Catalog,
    recipe: &Recipe,
    options: &BatchOptions,
    on_progress: &mut dyn FnMut(&ProgressEvent),
) -> RunReport {
    let started = Instant::now();
    let mut scenes = Vec::new();
    let mut errors = Vec::new();
    let mut totals = Totals::default();
    let mut cancelled = false;

    for index in 0..recipe.scenes {
        if options.cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }

        let scene_started = Instant::now();
        let built = compose(catalog, recipe, index).and_then(|scene| {
            let cloud = scene_cloud(&scene);
            let stats = scene_stats(index, &scene, &cloud, String::new(), 0, 0);
            let bytes = write_scene(&options.output_dir, &cloud, &stats)?;
            Ok((scene, stats, bytes))
        });

        match built {
            Ok((_scene, stats, bytes)) => {
                let elapsed_ms = scene_started.elapsed().as_millis() as u64;
                let mut stats = stats;
                stats.elapsed_ms = elapsed_ms;
                stats.bytes = bytes;
                let paths = super::export::scene_paths(&options.output_dir, index);
                stats.file = paths
                    .xyz
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();

                totals.add(&stats);
                let event = ProgressEvent {
                    job_id: options.job_id.clone(),
                    scenes_done: totals.scenes,
                    scenes_total: recipe.scenes,
                    points_total: totals.points,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    last: stats.clone(),
                };
                scenes.push(stats);
                on_progress(&event);
            }
            Err(error) => {
                errors.push(SceneError {
                    index,
                    message: error.to_string(),
                });
            }
        }
    }

    totals.elapsed_ms = started.elapsed().as_millis() as u64;

    let report = RunReport {
        job_id: options.job_id.clone(),
        output_dir: options.output_dir.to_string_lossy().into_owned(),
        recipe: recipe.clone(),
        resolved: resolved_presets(catalog, recipe),
        scenes,
        totals,
        errors,
        cancelled,
    };

    if let Err(error) = write_run_report(&options.output_dir, &report) {
        eprintln!("не удалось записать run.json: {error}");
    }

    report
}

/// Разрешённые имена пресетов рецепта: id → `"категория/имя"`.
///
/// Имя берётся из каталога, чтобы датасет читался без доступа к нему. Если
/// пресета в каталоге уже нет, остаётся id — отчёт не теряет запись из-за
/// одного отсутствующего имени.
fn resolved_presets(catalog: &Catalog, recipe: &Recipe) -> std::collections::BTreeMap<String, String> {
    let mut resolved = std::collections::BTreeMap::new();
    let mut insert = |category: &str, id: &str| {
        if id.is_empty() {
            return;
        }
        let name = catalog
            .category(category)
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| id.to_string());
        resolved.insert(id.to_string(), format!("{category}/{name}"));
    };

    insert("landscapes", &recipe.landscape);
    for element in &recipe.elements {
        insert(element.kind.category(), &element.preset_id);
    }
    resolved
}

/// Удобный конструктор статистики для отчётов вне батча.
pub fn stats_of(index: u32, seed: u64, points: u64) -> SceneStats {
    SceneStats {
        index,
        seed,
        file: format!("scene-{index:04}.xyz"),
        bytes: 0,
        points,
        by_class: Default::default(),
        objects: Default::default(),
        bbox: Default::default(),
        elapsed_ms: 0,
        frame: super::stats::FRAME_Z_UP.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use super::*;
    use crate::dataset::catalog::load_catalog;
    use crate::dataset::recipe::{ElementKind, ElementSpec};
    use crate::dataset::test_support::repo_presets;

    fn target_dir(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(name)
    }

    fn tiny_recipe(scenes: u32) -> Recipe {
        Recipe {
            scenes,
            seed: 123,
            landscape: "plain".into(),
            max_slope_deg: 30.0,
            elements: vec![ElementSpec {
                kind: ElementKind::Grass,
                preset_id: "sparse-grass".into(),
                count: 4,
            }],
        }
    }

    fn options(dir: PathBuf, job_id: &str) -> BatchOptions {
        BatchOptions {
            output_dir: dir,
            job_id: job_id.to_string(),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    #[test]
    fn batch_writes_files_and_reports_progress() {
        let dir = target_dir("dataset-job-tests");
        let _ = std::fs::remove_dir_all(&dir);
        let catalog = load_catalog(&repo_presets());
        let mut seen = Vec::new();
        let report = run_batch(&catalog, &tiny_recipe(2), &options(dir.clone(), "test"), &mut |e| {
            seen.push((e.scenes_done, e.points_total))
        });
        assert_eq!(seen.len(), 2);
        assert!(seen[1].0 == 2 && seen[1].1 >= seen[0].1);
        assert_eq!(report.scenes.len(), 2);
        assert!(
            dir.join("scene-0000.xyz").is_file() && dir.join("scene-0001.xyz").is_file(),
            "обе сцены должны быть записаны"
        );
        let run_json = std::fs::read_to_string(dir.join("run.json")).unwrap();
        assert!(run_json.contains("\"recipe\""));
        assert!(dir.join("scene-0001.stats.json").is_file());
        assert!(!report.cancelled);
        assert_eq!(report.totals.scenes, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancel_after_first_scene_keeps_only_finished_files() {
        let dir = target_dir("dataset-job-cancel-mid");
        let _ = std::fs::remove_dir_all(&dir);
        let catalog = load_catalog(&repo_presets());
        let flag = Arc::new(AtomicBool::new(false));
        let flag_in_cb = flag.clone();
        let report = run_batch(
            &catalog,
            &tiny_recipe(3),
            &BatchOptions {
                output_dir: dir.clone(),
                job_id: "mid".into(),
                cancel: flag.clone(),
            },
            &mut |_| flag_in_cb.store(true, Ordering::Relaxed),
        );
        assert!(report.cancelled);
        assert_eq!(report.scenes.len(), 1);
        assert!(dir.join("scene-0000.xyz").is_file());
        assert!(!dir.join("scene-0001.xyz").exists() && !dir.join("scene-0002.xyz").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn failing_scenes_are_collected_and_run_json_still_written() {
        let dir = target_dir("dataset-job-fail");
        let _ = std::fs::remove_dir_all(&dir);
        let mut catalog = load_catalog(&repo_presets());
        catalog.landscapes.clear();
        let report = run_batch(&catalog, &tiny_recipe(2), &options(dir.clone(), "fail"), &mut |_| {});
        assert_eq!(report.errors.len(), 2);
        assert!(report.scenes.is_empty());
        assert!(!report.cancelled);
        assert!(dir.join("run.json").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn progress_counters_are_monotonic() {
        let dir = target_dir("dataset-job-progress");
        let _ = std::fs::remove_dir_all(&dir);
        let catalog = load_catalog(&repo_presets());
        let mut points = Vec::new();
        run_batch(&catalog, &tiny_recipe(3), &options(dir.clone(), "p"), &mut |e| {
            points.push(e.points_total)
        });
        assert_eq!(points.len(), 3);
        assert!(points.windows(2).all(|w| w[1] >= w[0]), "{points:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
