//! Сборка единого облака сцены.
//!
//! Генераторы отдают облака объектов в локальных координатах: точка основания
//! у них — начало координат. Поэтому каждое облако клонируется и переводится
//! в мировые координаты матрицей «поворот вокруг Z + сдвиг в точку рельефа».
//! Порядок обхода — векторы сцены, а не хеш-таблицы, иначе облако менялось бы
//! от запуска к запуску.

use bevy::math::{Mat4, Quat};
use generation_core::PointCloud;

use super::scene::{Placed, Scene};

/// Единое облако сцены: грунт, вода, затем здания, деревья, кусты и трава.
///
/// Точки объектов переводятся в мир той же матрицей, что и `Transform` в
/// визуализации, поэтому координаты совпадают с тем, что показывает
/// существующий просмотрщик.
pub fn scene_cloud(scene: &Scene) -> PointCloud {
    let mut cloud = scene.landscape.ground_cloud.clone();
    cloud.merge(&scene.landscape.water_cloud);

    for placed in &scene.buildings {
        append(&mut cloud, &placed.item.point_cloud, placed);
    }
    for placed in &scene.trees {
        append(&mut cloud, &placed.item.point_cloud, placed);
    }
    for placed in &scene.bushes {
        append(&mut cloud, &placed.item.point_cloud, placed);
    }
    for placed in &scene.grass {
        append(&mut cloud, &placed.item.point_cloud, placed);
    }

    cloud
}

/// Добавить облако объекта, переведённое в мировые координаты.
fn append<T>(target: &mut PointCloud, source: &PointCloud, placed: &Placed<T>) {
    let mut local = source.clone();
    local.transform_by(world_matrix(placed));
    target.merge(&local);
}

/// Матрица перевода локальных координат объекта в мировые.
fn world_matrix<T>(placed: &Placed<T>) -> Mat4 {
    Mat4::from_rotation_translation(
        Quat::from_rotation_z(placed.rotation),
        placed.position,
    )
}
