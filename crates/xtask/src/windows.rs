//! Ярлыки Windows: `.lnk` создаются через `WScript.Shell` в PowerShell.
//!
//! Текст скрипта формируется здесь и выполняется отдельно, поэтому его можно
//! проверить тестом без Windows. Пути попадают в строки PowerShell в одинарных
//! кавычках (`'` → `''`), а `"` в пути отвергается: она рвёт аргументы `cmd /c`.

use std::path::{Path, PathBuf};

use crate::apps::LaunchApp;
use crate::paths::{CargoPath, crate_dir};

/// Каталог меню «Пуск», куда кладутся ярлыки.
pub fn start_menu_dir(appdata: Option<&Path>) -> Option<PathBuf> {
    appdata.map(|dir| dir.join("Microsoft/Windows/Start Menu/Programs/LiDAR"))
}

/// Текст PowerShell-скрипта, создающего `.lnk`.
pub fn shortcut_script(
    app: &LaunchApp,
    crate_root: &Path,
    cargo: &CargoPath,
    lnk_path: &Path,
    icon: &Path,
) -> Result<String, String> {
    let app_dir = crate_dir(crate_root, app.package);
    for path in [app_dir.as_path(), Path::new(&cargo.0), lnk_path, icon] {
        let text = path.display().to_string();
        if text.contains('"') {
            return Err(format!("путь содержит '\"': {text}"));
        }
    }

    let app_dir = app_dir.display().to_string();
    let working_dir = app_dir.replace('/', "\\");
    let cargo = cargo.0.replace('/', "\\");
    // Целью ярлыка обязан быть `cmd.exe`: у ярлыка нет командной строки, поэтому
    // `cd /d … && cargo run` требует именно её. Если целью сделать сам cargo,
    // `/c` уедет ему в аргументы и запуск упадёт.
    let arguments = format!(
        "/c cd /d \"{working_dir}\" && \"{cargo}\" run -p {}",
        app.package
    );

    Ok(format!(
        "$shell = New-Object -ComObject WScript.Shell\n\
         $s = $shell.CreateShortcut('{lnk}')\n\
         $s.TargetPath = 'cmd.exe'\n\
         $s.Arguments = '{arguments}'\n\
         $s.WorkingDirectory = '{working}'\n\
         $s.IconLocation = '{icon}'\n\
         $s.Description = '{description}'\n\
         $s.Save()\n",
        lnk = quote(&lnk_path.display().to_string()),
        arguments = quote(&arguments),
        working = quote(&working_dir),
        icon = quote(&icon.display().to_string()),
        description = quote(app.comment),
    ))
}

/// Экранировать строку для одинарных кавычек PowerShell.
fn quote(text: &str) -> String {
    text.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    fn app() -> crate::apps::LaunchApp {
        crate::apps::LaunchApp {
            id: "plants",
            title: "LiDAR Plant Generator",
            package: "plants-generator",
            comment: "Требует Rust",
        }
    }

    #[test]
    fn shortcut_script_quotes_path() {
        let script = super::shortcut_script(
            &app(),
            std::path::Path::new("C:/My Projects/lidar"),
            &crate::paths::CargoPath("C:/Users/u/.cargo/bin/cargo.exe".into()),
            std::path::Path::new("C:/Users/u/AppData/Roaming/LiDAR/LiDAR Plant Generator.lnk"),
            std::path::Path::new("C:/icon.ico"),
        )
        .unwrap();
        assert!(script.contains("WScript.Shell"));
        assert!(script.contains("CreateShortcut"));
        assert!(script.contains("cd /d"));
        assert!(script.contains("plants-generator"));
        assert!(script.contains("LiDAR Plant Generator.lnk"));
        assert!(
            script.contains("$s.TargetPath = '") && script.contains("cmd.exe"),
            "целью ярлыка обязан быть cmd.exe, иначе /c попадёт в cargo: {script}"
        );
    }

    #[test]
    fn start_menu_dir_sits_under_appdata() {
        assert_eq!(
            super::start_menu_dir(Some(std::path::Path::new("C:/Users/u/AppData/Roaming"))),
            Some(std::path::PathBuf::from(
                "C:/Users/u/AppData/Roaming/Microsoft/Windows/Start Menu/Programs/LiDAR"
            ))
        );
    }
}
