//! Превью сцены: сжатая копия облака точек для просмотра в окне.
//!
//! Просмотр не должен тянуть наружу полный файл: сцена на четыреста пучков
//! травы и десяток зданий даёт сотни тысяч точек, а окно рисует их точками
//! радиусом в один пиксель. Поэтому точки прореживаются по сетке, координаты
//! округляются, а класс заменяется кодом из палитры.
//!
//! Прореживание детерминировано: точки берутся равномерным шагом по облаку,
//! поэтому один и тот же рецепт даёт одно и то же превью, а все классы
//! сохраняют свою долю. Сетка по объёму для этого не годится: сцена это
//! плоский рельеф 100 на 100 метров, и кубическая ячейка оставляла бы одну
//! точку на несколько метров, теряя почти всё облако.

use generation_core::PointCloud;
use serde::Serialize;

use super::recipe::Recipe;
use super::scene::Scene;
use super::stats::{BBox, SceneStats};

/// Сколько точек максимум отдаётся в превью.
pub const PREVIEW_MAX_POINTS: usize = 60_000;

/// Превью одной сцены.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenePreview {
    /// Индекс сцены.
    pub index: u32,
    /// Сид сцены.
    pub seed: u64,
    /// Система координат (`"z-up"`).
    pub frame: String,
    /// Границы облака.
    pub bbox: BBox,
    /// Точки: x, y, z и код класса, по четыре числа на точку.
    pub points: Vec<f32>,
    /// Коды классов, встречающиеся в превью.
    pub classes: Vec<u8>,
    /// Сколько точек в полном облаке.
    pub points_total: u64,
    /// Сколько точек попало в превью.
    pub points_shown: u64,
    /// Статистика сцены без имени файла и размеров.
    pub stats: SceneStats,
}

impl ScenePreview {
    /// Превью сжатой сцены, собранное из её облака.
    pub fn new(index: u32, scene: &Scene, cloud: &PointCloud) -> Self {
        let points = decimate(cloud, PREVIEW_MAX_POINTS);
        let mut classes: Vec<u8> = points.chunks_exact(4).map(|p| p[3] as u8).collect();
        classes.sort_unstable();
        classes.dedup();

        let mut stats = super::stats::scene_stats(index, scene, cloud, String::new(), 0, 0);
        stats.file = String::new();
        let points_shown = (points.len() / 4) as u64;

        Self {
            index,
            seed: scene.seed.value(),
            frame: super::stats::FRAME_Z_UP.to_string(),
            bbox: stats.bbox,
            points_shown,
            points,
            classes,
            points_total: cloud.points.len() as u64,
            stats,
        }
    }
}

/// Прореженные точки облака в виде `[x, y, z, класс, ...]`.
///
/// Шаг по облаку равномерный и не зависит от порядка классов, поэтому превью
/// воспроизводимо и содержит ровно `limit` точек.
pub fn decimate(cloud: &PointCloud, limit: usize) -> Vec<f32> {
    let total = cloud.points.len();
    if limit == 0 || total == 0 {
        return Vec::new();
    }
    if total <= limit {
        return flatten(cloud, |_| true);
    }

    let step = total as f64 / limit as f64;
    let mut out = Vec::with_capacity(limit * 4);
    for index in 0..limit {
        let source = ((index as f64) * step) as usize;
        let point = &cloud.points[source.min(total - 1)];
        out.push(point.position.x);
        out.push(point.position.y);
        out.push(point.position.z);
        out.push(point.class.code() as f32);
    }
    out
}

fn flatten(cloud: &PointCloud, keep: impl Fn(usize) -> bool) -> Vec<f32> {
    let mut out = Vec::with_capacity(cloud.points.len() * 4);
    for (index, point) in cloud.points.iter().enumerate() {
        if keep(index) {
            out.push(point.position.x);
            out.push(point.position.y);
            out.push(point.position.z);
            out.push(point.class.code() as f32);
        }
    }
    out
}

/// Проверить рецепт и собрать превью сцены `index`.
///
/// Отдельная точка входа для кнопки «Показать сцену»: она не пишет файлов и
/// не занимает задачу генерации, поэтому её можно нажимать во время запуска.
pub fn preview_scene(
    catalog: &super::catalog::Catalog,
    recipe: &Recipe,
    index: u32,
) -> Result<ScenePreview, crate::error::RunnerError> {
    let issues = super::recipe::validate(recipe, catalog);
    if !issues.is_empty() {
        return Err(crate::error::RunnerError::Validation(issues));
    }
    let scene = super::scene::compose(catalog, recipe, index)?;
    let cloud = super::merge::scene_cloud(&scene);
    Ok(ScenePreview::new(index, &scene, &cloud))
}

#[cfg(test)]
mod tests {
    use generation_core::PointClass;
    use generation_core::point_clouds::PointCloud;

    use super::*;

    fn flat_cloud(count: usize) -> PointCloud {
        let mut cloud = PointCloud::new(PointClass::Ground);
        for index in 0..count {
            let t = index as f32;
            cloud.push(bevy::math::Vec3::new(t * 0.01, t * 0.01, 0.0));
        }
        cloud
    }

    #[test]
    fn small_clouds_are_kept_whole() {
        let cloud = flat_cloud(100);
        let points = decimate(&cloud, PREVIEW_MAX_POINTS);
        assert_eq!(points.len(), 400);
        assert_eq!(points[3], PointClass::Ground.code() as f32);
    }

    #[test]
    fn big_clouds_fill_the_limit_exactly() {
        let cloud = flat_cloud(200_000);
        let points = decimate(&cloud, 5_000);
        assert_eq!(points.len(), 5_000 * 4);
    }

    #[test]
    fn thinning_keeps_every_class_represented() {
        let mut cloud = PointCloud::new(PointClass::Ground);
        for index in 0..10_000 {
            let class = if index % 10 == 0 {
                PointClass::Building
            } else {
                PointClass::Ground
            };
            cloud.points.push(generation_core::point_clouds::Point::new(
                bevy::math::Vec3::new(index as f32, 0.0, 0.0),
                class,
            ));
        }
        let points = decimate(&cloud, 1_000);
        let buildings = points
            .chunks_exact(4)
            .filter(|p| p[3] == PointClass::Building.code() as f32)
            .count();
        assert!(buildings > 50, "зданий в превью: {buildings}");
    }

    #[test]
    fn thinning_is_deterministic() {
        let cloud = flat_cloud(50_000);
        assert_eq!(decimate(&cloud, 3_000), decimate(&cloud, 3_000));
    }

    #[test]
    fn preview_reports_both_totals_and_the_frame() {
        let catalog = super::super::catalog::load_catalog(
            &super::super::test_support::repo_presets(),
        );
        let recipe = Recipe {
            scenes: 1,
            seed: 3,
            landscape: "plain".into(),
            max_slope_deg: 25.0,
            elements: vec![super::super::recipe::ElementSpec {
                kind: super::super::recipe::ElementKind::Building,
                preset_id: "rectangular".into(),
                count: 2,
            }],
        };
        let preview = preview_scene(&catalog, &recipe, 0).expect("превью собирается");
        assert_eq!(preview.frame, super::super::stats::FRAME_Z_UP);
        assert_eq!(preview.index, 0);
        assert_eq!(preview.seed, preview.stats.seed);
        assert!(preview.points_shown <= PREVIEW_MAX_POINTS as u64);
        assert_eq!(preview.points.len() as u64, preview.points_shown * 4);
        assert!(preview.points_total >= preview.points_shown);
    }

    #[test]
    fn invalid_recipes_are_rejected_before_composition() {
        let catalog = super::super::catalog::load_catalog(
            &super::super::test_support::repo_presets(),
        );
        let recipe = Recipe {
            scenes: 1,
            seed: 3,
            landscape: "no-such-landscape".into(),
            max_slope_deg: 25.0,
            elements: Vec::new(),
        };
        assert!(preview_scene(&catalog, &recipe, 0).is_err());
    }
}
