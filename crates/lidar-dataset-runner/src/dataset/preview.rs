//! Превью сцены: сжатая копия облака точек для просмотра в окне.
//!
//! Просмотр не должен тянуть наружу полный файл: сцена на четыреста пучков
//! травы и десяток зданий даёт сотни тысяч точек, а окно рисует их точками
//! радиусом в один пиксель. Поэтому точки прореживаются по сетке, координаты
//! округляются, а класс заменяется кодом из палитры.
//!
//! Прореживание детерминировано: укладываем точки в ячейки сетки и оставляем
//! первую точку ячейки в порядке обхода облака. Один и тот же рецепт даёт
//! одно и то же превью, и порядок точек не зависит от хеш-таблиц.

use std::collections::HashMap;

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
/// Поверх лимита в одну ячейку сетки попадает одна точка: сетка подбирается
/// по габаритам, чтобы ячеек было примерно столько же, сколько нужно точек.
pub fn decimate(cloud: &PointCloud, limit: usize) -> Vec<f32> {
    if limit == 0 || cloud.points.is_empty() {
        return Vec::new();
    }
    if cloud.points.len() <= limit {
        return flatten(cloud, |_| true);
    }

    let (min, max) = bounds(cloud);
    let span = (max - min).max(bevy::math::Vec3::splat(1.0));
    // Ячейка кубическая: шаг подбирается так, чтобы по объёму вышло около
    // `limit` ячеек. Кубическая сетка не сгущает точки вдоль одной оси, как
    // это сделала бы сетка в экранных координатах.
    let cells_per_axis = (limit as f32).cbrt().ceil().max(1.0);
    let cell = span / bevy::math::Vec3::splat(cells_per_axis);

    let mut seen: HashMap<(i64, i64, i64), ()> = HashMap::new();
    let mut out = Vec::with_capacity(limit * 4);
    for point in &cloud.points {
        if out.len() / 4 >= limit {
            break;
        }
        let relative = (point.position - min) / cell;
        let key = (
            relative.x.floor() as i64,
            relative.y.floor() as i64,
            relative.z.floor() as i64,
        );
        if seen.insert(key, ()).is_none() {
            out.push(point.position.x);
            out.push(point.position.y);
            out.push(point.position.z);
            out.push(point.class.code() as f32);
        }
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

fn bounds(cloud: &PointCloud) -> (bevy::math::Vec3, bevy::math::Vec3) {
    let mut min = bevy::math::Vec3::splat(f32::INFINITY);
    let mut max = bevy::math::Vec3::splat(f32::NEG_INFINITY);
    for point in &cloud.points {
        min = min.min(point.position);
        max = max.max(point.position);
    }
    (min, max)
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
    fn big_clouds_are_thinned_below_the_limit() {
        let cloud = flat_cloud(200_000);
        let points = decimate(&cloud, 5_000);
        let shown = points.len() / 4;
        assert!(shown <= 5_000, "показано {shown}");
        assert!(shown > 0);
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
