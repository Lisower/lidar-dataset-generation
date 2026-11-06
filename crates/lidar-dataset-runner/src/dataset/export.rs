//! Экспорт сцены: `.xyz`, `stats.json` и `run.json`.
//!
//! Облако пишется существующим [`write_xyz`], поэтому файл побайтово совпадает
//! с тем, что уже умеет проект (формат `класс x y z`, без заголовка).
//!
//! Сцена пишется целиком или не пишется вовсе: сначала `.xyz`, затем
//! `stats.json`. Заголовочный `run.json` пишется только один раз, после
//! последней сцены, поэтому его наличие означает завершённый (или честно
//! прерванный) запуск, а не полузаписанный каталог.

use std::path::{Path, PathBuf};

use generation_core::PointCloud;
use generation_core::point_clouds::export::write_xyz;

use super::stats::{RunReport, SceneStats};
use crate::error::RunnerError;

/// Пути файлов одной сцены.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenePaths {
    /// Файл облака точек.
    pub xyz: PathBuf,
    /// Файл статистики.
    pub stats: PathBuf,
}

/// Пути файлов сцены `index` в каталоге `output_dir`.
pub fn scene_paths(output_dir: &Path, index: u32) -> ScenePaths {
    ScenePaths {
        xyz: output_dir.join(format!("scene-{index:04}.xyz")),
        stats: output_dir.join(format!("scene-{index:04}.stats.json")),
    }
}

/// Записать облако сцены и её статистику; вернуть размер `.xyz` в байтах.
///
/// `stats` не обязана содержать правильные `file` и `bytes`: они
/// выставляются здесь по фактической записи, чтобы файл не расходился с
/// диском.
pub fn write_scene(
    output_dir: &Path,
    cloud: &PointCloud,
    stats: &SceneStats,
) -> Result<u64, RunnerError> {
    std::fs::create_dir_all(output_dir)
        .map_err(|source| RunnerError::io(output_dir, source))?;

    let paths = scene_paths(output_dir, stats.index);
    write_xyz(cloud, &paths.xyz).map_err(|source| RunnerError::io(&paths.xyz, source))?;

    let bytes = std::fs::metadata(&paths.xyz)
        .map_err(|source| RunnerError::io(&paths.xyz, source))?
        .len();

    let mut saved = stats.clone();
    saved.file = file_name(&paths.xyz);
    saved.bytes = bytes;

    let json = serde_json::to_string_pretty(&saved).map_err(|source| RunnerError::Io {
        path: paths.stats.clone(),
        source: std::io::Error::other(source),
    })?;
    std::fs::write(&paths.stats, json).map_err(|source| RunnerError::io(&paths.stats, source))?;

    Ok(bytes)
}

/// Записать отчёт запуска в `run.json`.
pub fn write_run_report(output_dir: &Path, report: &RunReport) -> Result<(), RunnerError> {
    std::fs::create_dir_all(output_dir)
        .map_err(|source| RunnerError::io(output_dir, source))?;
    let path = output_dir.join("run.json");
    let json = serde_json::to_string_pretty(report).map_err(|source| RunnerError::Io {
        path: path.clone(),
        source: std::io::Error::other(source),
    })?;
    std::fs::write(&path, json).map_err(|source| RunnerError::io(&path, source))
}

/// Имя файла без каталогов — так оно попадает в статистику.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use bevy::math::Vec3;
    use generation_core::point_clouds::export::to_xyz;
    use generation_core::{Point, PointClass, PointCloud};

    use super::*;
    use crate::dataset::stats::{BBox, FRAME_Z_UP};

    fn target_dir(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(name)
    }

    fn sample_stats(points: u64) -> SceneStats {
        SceneStats {
            index: 0,
            seed: 1,
            file: "scene-0000.xyz".into(),
            bytes: 0,
            points,
            by_class: Default::default(),
            objects: Default::default(),
            bbox: BBox {
                min: [0.0; 3],
                max: [0.0; 3],
            },
            elapsed_ms: 0,
            frame: FRAME_Z_UP.to_string(),
        }
    }

    #[test]
    fn scene_is_written_as_xyz_with_matching_stats() {
        let dir = target_dir("dataset-export-tests");
        let _ = std::fs::remove_dir_all(&dir);
        let mut cloud = PointCloud::new(PointClass::Ground);
        cloud
            .points
            .push(Point::new(Vec3::new(1.0, -2.0, 0.5), PointClass::Building));
        cloud
            .points
            .push(Point::new(Vec3::new(0.0, 0.0, 0.0), PointClass::Ground));
        let stats = sample_stats(cloud.len() as u64);
        let bytes = write_scene(&dir, &cloud, &stats).unwrap();
        assert_eq!(
            bytes,
            std::fs::metadata(dir.join("scene-0000.xyz")).unwrap().len()
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("scene-0000.xyz")).unwrap(),
            to_xyz(&cloud)
        );
        let saved: SceneStats =
            serde_json::from_str(&std::fs::read_to_string(dir.join("scene-0000.stats.json")).unwrap())
                .unwrap();
        assert_eq!(saved.points, 2);
        assert_eq!(saved.bytes, bytes);
        assert_eq!(saved.file, "scene-0000.xyz");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unwritable_output_is_an_error_not_a_panic() {
        let dir = target_dir("dataset-export-file");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.parent().unwrap()).unwrap();
        std::fs::write(&dir, b"not a directory").unwrap();
        let cloud = PointCloud::new(PointClass::Ground);
        let stats = sample_stats(0);
        assert!(matches!(
            write_scene(&dir, &cloud, &stats),
            Err(RunnerError::Io { .. })
        ));
        let _ = std::fs::remove_file(&dir);
    }

    #[test]
    #[ignore = "manual: измеряет стоимость записи сцены, гонять вручную"]
    fn write_cost_share_of_a_forest_scene() {
        use crate::dataset::catalog::load_catalog;
        use crate::dataset::merge::scene_cloud;
        use crate::dataset::recipe::{ElementKind, ElementSpec, Recipe};
        use crate::dataset::scene::compose;
        use crate::dataset::stats::scene_stats;
        use crate::dataset::test_support::repo_presets;

        let dir = target_dir("dataset-export-cost");
        let _ = std::fs::remove_dir_all(&dir);
        let catalog = load_catalog(&repo_presets());
        let recipe = Recipe {
            scenes: 1,
            seed: 12648430,
            landscape: "plain".into(),
            max_slope_deg: 30.0,
            elements: vec![
                ElementSpec { kind: ElementKind::Tree, preset_id: "deciduous-tree".into(), count: 35 },
                ElementSpec { kind: ElementKind::Bush, preset_id: "bush".into(), count: 38 },
                ElementSpec { kind: ElementKind::Grass, preset_id: "tall-grass".into(), count: 200 },
            ],
        };
        let started = std::time::Instant::now();
        let scene = compose(&catalog, &recipe, 0).unwrap();
        let cloud = scene_cloud(&scene);
        let compose_ms = started.elapsed().as_millis();
        let stats = scene_stats(0, &scene, &cloud, "scene-0000.xyz".into(), 0, 0);
        let write_started = std::time::Instant::now();
        let bytes = write_scene(&dir, &cloud, &stats).unwrap();
        let write_ms = write_started.elapsed().as_millis();
        eprintln!(
            "точек: {}, сборка: {compose_ms} мс, запись: {write_ms} мс ({bytes} байт), доля записи: {:.0}%",
            cloud.len(),
            write_ms as f64 * 100.0 / (compose_ms + write_ms).max(1) as f64
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_report_round_trips_json() {
        let dir = target_dir("dataset-export-run");
        let _ = std::fs::remove_dir_all(&dir);
        let report = RunReport {
            job_id: "test".into(),
            output_dir: dir.to_string_lossy().into_owned(),
            recipe: crate::dataset::recipe::Recipe {
                scenes: 1,
                seed: 3,
                landscape: "plain".into(),
                max_slope_deg: 20.0,
                elements: Vec::new(),
            },
            resolved: Default::default(),
            scenes: vec![sample_stats(2)],
            totals: Default::default(),
            errors: Vec::new(),
            cancelled: false,
        };
        write_run_report(&dir, &report).unwrap();
        let text = std::fs::read_to_string(dir.join("run.json")).unwrap();
        let saved: RunReport = serde_json::from_str(&text).unwrap();
        assert_eq!(saved.job_id, "test");
        assert_eq!(saved.recipe.seed, 3);
        assert_eq!(saved.scenes.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
