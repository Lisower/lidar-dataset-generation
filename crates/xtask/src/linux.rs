//! Ярлыки Linux: `.desktop`-файлы в каталоге приложений.
//!
//! `Exec` в `.desktop` не проходит через оболочку, поэтому `cd … && cargo run`
//! заворачивается в `sh -c '…'`. Кавычки внутри одинарных не экранируются, так
//! что путь с апострофом отвергается с понятной ошибкой, а не даёт битый ярлык.

use std::path::{Path, PathBuf};

use crate::apps::LaunchApp;
use crate::paths::{CargoPath, crate_dir};

/// Каталог, куда складываются ярлыки приложений.
pub fn applications_dir(data_home: Option<&Path>, home: Option<&Path>) -> Option<PathBuf> {
    let base = data_home
        .map(Path::to_path_buf)
        .or_else(|| home.map(|home| home.join(".local/share")))?;
    Some(base.join("applications"))
}

/// Каталог рабочего стола, если он известен.
pub fn desktop_dir(xdg_desktop: Option<&Path>, home: Option<&Path>) -> Option<PathBuf> {
    xdg_desktop
        .map(Path::to_path_buf)
        .or_else(|| home.map(|home| home.join("Desktop")))
}

/// Содержимое `.desktop`-файла.
pub fn desktop_entry(
    app: &LaunchApp,
    crate_root: &Path,
    cargo: &CargoPath,
    icon: &Path,
) -> Result<String, String> {
    let app_dir = crate_dir(crate_root, app.package);
    check_path(&app_dir)?;
    check_path(Path::new(&cargo.0))?;
    check_path(icon)?;

    let command = format!(
        "cd \"{}\" && \"{}\" run -p {}",
        app_dir.display(),
        cargo.0,
        app.package
    );
    let command = if command.contains('\'') {
        return Err(format!("путь содержит \"'\": {command}"));
    } else {
        command
    };

    Ok(format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name={}\n\
         Comment={}\n\
         Exec=sh -c '{command}'\n\
         Icon={}\n\
         Terminal=false\n\
         Categories=Utility;\n\
         StartupNotify=true\n",
        app.title,
        app.comment,
        icon.display()
    ))
}

/// Записать файл ярлыка, создав каталоги.
pub fn write_entry(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "не удалось создать каталог ярлыков {}: {error}",
                parent.display()
            )
        })?;
    }
    std::fs::write(path, content)
        .map_err(|error| format!("не удалось записать ярлык {}: {error}", path.display()))
}

/// Отвергнуть символы, которые нельзя писать в `.desktop`.
///
/// `%` — field code в `Exec` и `Icon`, `"` рвёт аргумент `sh -c`.
fn check_path(path: &Path) -> Result<(), String> {
    let text = path.display().to_string();
    for forbidden in ['%', '"'] {
        if text.contains(forbidden) {
            return Err(format!("путь содержит \"{forbidden}\": {text}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    fn app() -> crate::apps::LaunchApp {
        crate::apps::LaunchApp {
            id: "landscapes",
            title: "LiDAR Landscape Generator",
            package: "landscapes-generator",
            comment: "Требует Rust",
        }
    }

    #[test]
    fn desktop_entry_quotes_paths_with_spaces() {
        let entry = super::desktop_entry(
            &app(),
            std::path::Path::new("/home/u/My Projects/lidar"),
            &crate::paths::CargoPath("/home/u/.cargo/bin/cargo".into()),
            std::path::Path::new("/home/u/My Projects/lidar/icon.png"),
        )
        .unwrap();
        assert!(entry.contains("[Desktop Entry]"));
        assert!(entry.contains("Name=LiDAR Landscape Generator"));
        assert!(entry.contains("Terminal=false"));
        assert!(entry.contains("cd \"/home/u/My Projects/lidar/crates/landscapes-generator\""));
        assert!(entry.contains("\"/home/u/.cargo/bin/cargo\" run -p landscapes-generator"));
        assert!(entry.contains("Icon=/home/u/My Projects/lidar/icon.png"));
        entry.lines().for_each(|line| assert!(!line.contains('%'), "{line}"));
    }

    #[test]
    fn desktop_entry_rejects_percent_and_quote() {
        let percent = super::desktop_entry(
            &app(),
            std::path::Path::new("/home/u/50%/lidar"),
            &crate::paths::CargoPath("cargo".into()),
            std::path::Path::new("/icon.png"),
        )
        .unwrap_err();
        assert!(percent.contains('%'), "{percent}");

        let quote = super::desktop_entry(
            &app(),
            std::path::Path::new("/home/u/we\"ird/lidar"),
            &crate::paths::CargoPath("cargo".into()),
            std::path::Path::new("/icon.png"),
        )
        .unwrap_err();
        assert!(quote.contains('"'), "{quote}");
    }

    #[test]
    fn applications_dir_prefers_data_home() {
        assert_eq!(
            super::applications_dir(
                Some(std::path::Path::new("/xdg")),
                Some(std::path::Path::new("/home/u"))
            ),
            Some(std::path::PathBuf::from("/xdg/applications"))
        );
        assert_eq!(
            super::applications_dir(None, Some(std::path::Path::new("/home/u"))),
            Some(std::path::PathBuf::from("/home/u/.local/share/applications"))
        );
    }
}
