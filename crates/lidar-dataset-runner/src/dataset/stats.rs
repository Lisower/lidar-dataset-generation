//! Статистика сцены и запуска.
//!
//! Считается на бэкенде и кладётся рядом с данными (`scene-NNNN.stats.json`,
//! `run.json`), чтобы датасет был самодостаточным: по нему видно, что
//! запрошено, что реально поставлено, сколько точек каждого класса и в каких
//! границах лежит сцена. Все структуры сериализуются в camelCase, потому что
//! файл читают внешние инструменты, а не код проекта.

use std::collections::BTreeMap;

use generation_core::PointCloud;
use serde::{Deserialize, Serialize};

use super::recipe::Recipe;
use super::scene::Scene;

/// Система координат экспорта. Данные генераторов Z-up, поворота нет.
pub const FRAME_Z_UP: &str = "z-up";

/// Границы сцены по осям.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BBox {
    /// Минимумы по x, y, z.
    pub min: [f32; 3],
    /// Максимумы по x, y, z.
    pub max: [f32; 3],
}

/// Сколько объектов вида запрошено и сколько поставлено.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectCounts {
    /// Запрошено рецептом.
    pub requested: u32,
    /// Реально поставлено на сцену.
    pub placed: u32,
}

/// Статистика одной сцены.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneStats {
    /// Индекс сцены в запуске.
    pub index: u32,
    /// Сид сцены.
    pub seed: u64,
    /// Имя файла облака (относительно каталога вывода).
    pub file: String,
    /// Размер файла в байтах.
    pub bytes: u64,
    /// Всего точек.
    pub points: u64,
    /// Код класса → точек.
    pub by_class: BTreeMap<u8, u64>,
    /// Вид объекта → запрошено/поставлено.
    pub objects: BTreeMap<String, ObjectCounts>,
    /// Границы облака.
    pub bbox: BBox,
    /// Время сборки и записи сцены, мс.
    pub elapsed_ms: u64,
    /// Система координат (`"z-up"`).
    pub frame: String,
}

/// Событие прогресса: одна завершённая сцена.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    /// Идентификатор запуска.
    pub job_id: String,
    /// Сколько сцен уже записано.
    pub scenes_done: u32,
    /// Сколько сцен в запуске.
    pub scenes_total: u32,
    /// Точек записано всего.
    pub points_total: u64,
    /// Прошло с начала запуска, мс.
    pub elapsed_ms: u64,
    /// Статистика последней сцены.
    pub last: SceneStats,
}

/// Итоги запуска.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    /// Сколько сцен записано.
    pub scenes: u32,
    /// Всего точек.
    pub points: u64,
    /// Код класса → точек.
    pub by_class: BTreeMap<u8, u64>,
    /// Суммарный размер файлов, байт.
    pub bytes: u64,
    /// Время запуска, мс.
    pub elapsed_ms: u64,
}

impl Totals {
    /// Добавить статистику сцены к итогам.
    pub fn add(&mut self, stats: &SceneStats) {
        self.scenes += 1;
        self.points += stats.points;
        for (code, count) in &stats.by_class {
            *self.by_class.entry(*code).or_insert(0) += count;
        }
        self.bytes += stats.bytes;
    }
}

/// Ошибка одной сцены: запуск продолжается, но причина остаётся в отчёте.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneError {
    /// Индекс сцены.
    pub index: u32,
    /// Текст ошибки.
    pub message: String,
}

/// Отчёт о запуске.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunReport {
    /// Идентификатор запуска.
    pub job_id: String,
    /// Каталог вывода.
    pub output_dir: String,
    /// Рецепт, по которому шёл запуск (чтобы датасет воспроизводился из своих файлов).
    pub recipe: Recipe,
    /// id пресета → `"категория/имя"` для всех использованных пресетов.
    pub resolved: BTreeMap<String, String>,
    /// Статистика записанных сцен.
    pub scenes: Vec<SceneStats>,
    /// Итоги по запуску.
    pub totals: Totals,
    /// Ошибки сцен.
    pub errors: Vec<SceneError>,
    /// Запуск отменён пользователем.
    pub cancelled: bool,
}

/// Статистика сцены по её облаку.
pub fn scene_stats(
    index: u32,
    scene: &Scene,
    cloud: &PointCloud,
    file: String,
    bytes: u64,
    elapsed_ms: u64,
) -> SceneStats {
    let mut by_class: BTreeMap<u8, u64> = BTreeMap::new();
    for point in &cloud.points {
        *by_class.entry(point.class.code()).or_insert(0) += 1;
    }

    let mut objects: BTreeMap<String, ObjectCounts> = BTreeMap::new();
    let placed = [
        ("building", scene.buildings.len()),
        ("tree", scene.trees.len()),
        ("bush", scene.bushes.len()),
        ("grass", scene.grass.len()),
    ];
    for (key, count) in placed {
        let requested = scene.requested.get(key).copied().unwrap_or(0);
        objects.insert(
            key.to_string(),
            ObjectCounts {
                requested,
                placed: count as u32,
            },
        );
    }

    SceneStats {
        index,
        seed: scene.seed.value(),
        file,
        bytes,
        points: cloud.len() as u64,
        by_class,
        objects,
        bbox: bbox_of(cloud),
        elapsed_ms,
        frame: FRAME_Z_UP.to_string(),
    }
}

/// Границы облака; пустое облако даёт нули.
pub fn bbox_of(cloud: &PointCloud) -> BBox {
    let Some(first) = cloud.points.first() else {
        return BBox::default();
    };
    let mut min = [first.position.x, first.position.y, first.position.z];
    let mut max = min;
    for point in &cloud.points[1..] {
        let p = point.position;
        min[0] = min[0].min(p.x);
        min[1] = min[1].min(p.y);
        min[2] = min[2].min(p.z);
        max[0] = max[0].max(p.x);
        max[1] = max[1].max(p.y);
        max[2] = max[2].max(p.z);
    }
    BBox { min, max }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::catalog::load_catalog;
    use crate::dataset::merge::scene_cloud;
    use crate::dataset::recipe::{ElementKind, ElementSpec};
    use crate::dataset::scene::compose;
    use crate::dataset::test_support::repo_presets;

    #[test]
    fn statistics_match_the_cloud() {
        let catalog = load_catalog(&repo_presets());
        let recipe = Recipe {
            scenes: 1,
            seed: 5,
            landscape: "plain".into(),
            max_slope_deg: 30.0,
            elements: vec![
                ElementSpec {
                    kind: ElementKind::Building,
                    preset_id: "rectangular".into(),
                    count: 1,
                },
                ElementSpec {
                    kind: ElementKind::Tree,
                    preset_id: "deciduous-tree".into(),
                    count: 2,
                },
            ],
        };
        let scene = compose(&catalog, &recipe, 0).unwrap();
        let cloud = scene_cloud(&scene);
        let stats = scene_stats(0, &scene, &cloud, "scene-0000.xyz".into(), 1024, 5);
        assert_eq!(stats.points, cloud.len() as u64);
        assert_eq!(stats.by_class.values().sum::<u64>(), stats.points);
        assert_eq!(stats.frame, "z-up");
        let min_z = cloud
            .points
            .iter()
            .map(|p| p.position.z)
            .fold(f32::INFINITY, f32::min);
        assert!((stats.bbox.min[2] - min_z).abs() < 1e-4);
        assert_eq!(
            stats.objects["building"],
            ObjectCounts {
                requested: 1,
                placed: scene.buildings.len() as u32
            }
        );
        assert!(stats.objects["tree"].placed <= stats.objects["tree"].requested);
        let json = serde_json::to_string(&stats).unwrap();
        assert!(
            json.contains("\"byClass\"") && json.contains("\"frame\":\"z-up\""),
            "{json}"
        );
    }

    #[test]
    fn empty_cloud_has_zero_bbox() {
        let cloud = PointCloud::new(generation_core::PointClass::Ground);
        assert_eq!(bbox_of(&cloud), BBox::default());
    }

    #[test]
    fn totals_accumulate_class_counts() {
        let mut totals = Totals::default();
        let stats = SceneStats {
            index: 0,
            seed: 1,
            file: "a.xyz".into(),
            bytes: 10,
            points: 3,
            by_class: BTreeMap::from([(2u8, 2u64), (6, 1)]),
            objects: BTreeMap::new(),
            bbox: BBox::default(),
            elapsed_ms: 0,
            frame: FRAME_Z_UP.to_string(),
        };
        totals.add(&stats);
        totals.add(&stats);
        assert_eq!(totals.scenes, 2);
        assert_eq!(totals.points, 6);
        assert_eq!(totals.bytes, 20);
        assert_eq!(totals.by_class[&2], 4);
        assert_eq!(totals.by_class[&6], 2);
    }
}
