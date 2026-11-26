//! Ярлыки macOS: `.command`-скрипты в `~/Applications`.
//!
//! Finder открывает `.command` в Терминале, поэтому скрипт обязан сам
//! перейти в каталог крейта: рабочий каталог Finder произвольный.

use std::path::{Path, PathBuf};

use crate::apps::LaunchApp;
use crate::paths::{CargoPath, crate_dir};

/// Каталог, куда складываются ярлыки приложений.
pub fn applications_dir(home: Option<&Path>) -> Option<PathBuf> {
    home.map(|home| home.join("Applications/LiDAR"))
}

/// Содержимое `.command`-скрипта.
pub fn command_script(
    app: &LaunchApp,
    crate_root: &Path,
    cargo: &CargoPath,
) -> Result<String, String> {
    let app_dir = crate_dir(crate_root, app.package);
    for path in [app_dir.as_path(), Path::new(&cargo.0)] {
        let text = path.display().to_string();
        if text.contains('"') {
            return Err(format!("путь содержит '\"': {text}"));
        }
    }

    Ok(format!(
        "#!/bin/sh\ncd \"{}\" || exit 1\nexec \"{}\" run -p {}\n",
        app_dir.display(),
        cargo.0,
        app.package
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn command_script_quotes_path_and_execs_cargo() {
        let script = super::command_script(
            &crate::apps::LaunchApp {
                id: "terrains",
                title: "LiDAR Terrain Generator",
                package: "terrains-generator",
                comment: "Требует Rust",
            },
            std::path::Path::new("/Users/u/My Projects/lidar"),
            &crate::paths::CargoPath("/Users/u/.cargo/bin/cargo".into()),
        )
        .unwrap();
        assert!(script.starts_with("#!/bin/sh"));
        assert!(script.contains("cd \"/Users/u/My Projects/lidar/crates/terrains-generator\""));
        assert!(script.contains("\"/Users/u/.cargo/bin/cargo\" run -p terrains-generator"));
    }

    #[test]
    fn applications_dir_sits_under_home() {
        assert_eq!(
            super::applications_dir(Some(std::path::Path::new("/Users/u"))),
            Some(std::path::PathBuf::from("/Users/u/Applications/LiDAR"))
        );
    }
}
