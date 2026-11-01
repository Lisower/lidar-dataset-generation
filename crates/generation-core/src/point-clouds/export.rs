//! Текстовый экспорт облаков точек в формат `.xyz`.
//!
//! Каждая строка файла — одна точка в виде `класс x y z`: числовой код
//! класса из [`PointClass::code`] и координаты точки. Такой текст удобно
//! читать людям и легко парсить скриптами.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use super::PointCloud;

/// Сериализовать облако в текст формата `.xyz`, по одной точке на строку.
///
/// Формат строки: `класс x y z`, например `2 0.5 1.5 -0.25`.
pub fn to_xyz(cloud: &PointCloud) -> String {
    let mut out = String::with_capacity(cloud.len().saturating_mul(32));
    for point in &cloud.points {
        let p = &point.position;
        // `{}` для f32 печатает кратчайшее точное представление.
        out.push_str(&format!(
            "{} {} {} {}\n",
            point.class.code(),
            p.x,
            p.y,
            p.z
        ));
    }
    out
}

/// Записать облако точек в файл формата `.xyz`.
///
/// Недостающие родительские каталоги создаются автоматически.
pub fn write_xyz(cloud: &PointCloud, path: impl AsRef<Path>) -> std::io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut out = BufWriter::new(File::create(path)?);
    out.write_all(to_xyz(cloud).as_bytes())?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use bevy::math::Vec3;

    use super::*;
    use crate::point_clouds::{PointClass, PointCloud};

    /// Тестовое облако с известными точками и классами.
    fn sample() -> PointCloud {
        let mut cloud = PointCloud::new(PointClass::Ground);
        cloud.push(Vec3::ONE);
        cloud.points.push(crate::Point::new(
            Vec3::new(-1.5, 2.25, 3.125),
            PointClass::Building,
        ));
        cloud
    }

    #[test]
    fn xyz_lines_have_class_and_coords() {
        let text = to_xyz(&sample());
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some("2 1 1 1"));
        assert_eq!(lines.next(), Some("6 -1.5 2.25 3.125"));
        assert_eq!(lines.next(), None);
    }

    #[test]
    fn to_xyz_of_empty_cloud_is_empty() {
        let cloud = PointCloud::new(PointClass::LowNoise);
        assert_eq!(to_xyz(&cloud), "");
    }

    #[test]
    fn write_xyz_round_trips() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("point-cloud-export-tests");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("nested/cloud.xyz");
        let cloud = sample();
        write_xyz(&cloud, &path).unwrap();

        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert_eq!(on_disk, to_xyz(&cloud));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
