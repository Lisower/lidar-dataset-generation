//! Команда `launch-shortcuts`: создание и удаление ярлыков запуска.
//!
//! Ярлык — dev-инструмент: он требует Rust, первый запуск собирает зависимости,
//! поэтому все шесть приложений запускаются одинаково — `cargo run -p <крейт>`
//! из каталога крейта. Собранного бинаря на чистом checkout может не быть, а UI
//! раннера вообще требует отдельного `npm run build`.

use std::path::{Path, PathBuf};

use crate::apps::{self, LaunchApp, Selector};
use crate::manifest::{self, Manifest, ManifestEntry, Platform};
use crate::paths::CargoPath;
use crate::shell::Shell;
use crate::{linux, macos, windows};

/// Разобранные флаги команды.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchOptions {
    /// Какие приложения нужны.
    pub apps: Selector,
    /// Дополнительно положить копию на рабочий стол.
    pub desktop: bool,
    /// Удалить созданные ярлыки вместо создания.
    pub remove: bool,
}

/// Окружение, в котором работает команда.
///
/// Явные поля вместо чтения переменных прямо в коде: иначе тест нельзя
/// запустить, не трогая настоящий каталог ярлыков пользователя.
pub struct Env<'a> {
    /// Корень репозитория.
    pub crate_root: &'a Path,
    /// Платформа.
    pub platform: Platform,
    /// `XDG_DATA_HOME`.
    pub xdg_data_home: Option<PathBuf>,
    /// `HOME` или `USERPROFILE`.
    pub home: Option<PathBuf>,
    /// `APPDATA`.
    pub appdata: Option<PathBuf>,
    /// `XDG_DESKTOP_DIR`.
    pub xdg_desktop: Option<PathBuf>,
    /// `CARGO`, если задан.
    pub cargo: Option<String>,
    /// Чем запускать сценарии, создающие ярлык.
    pub shell: &'a dyn Shell,
}

/// Разобрать аргументы команды.
pub fn parse_args(args: &[String]) -> Result<LaunchOptions, String> {
    let mut options = LaunchOptions {
        apps: Selector::All,
        desktop: false,
        remove: false,
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--apps" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "флаг --apps требует значение: runner, generators или all".to_string())?;
                options.apps = Selector::from_str(value)?;
            }
            "--desktop" => options.desktop = true,
            "--remove" => options.remove = true,
            other => return Err(format!("неизвестный флаг: {other}")),
        }
    }
    Ok(options)
}

/// Создать ярлыки; вернуть пути всех записанных файлов.
pub fn run(options: &LaunchOptions, env: &Env) -> Result<Vec<PathBuf>, String> {
    let cargo = CargoPath(env.cargo.clone().unwrap_or_else(|| "cargo".to_string()));
    let icon = icon_path(env.crate_root);
    let mut written = Vec::new();
    let mut entries = Vec::new();

    for app in apps::select_apps(options.apps) {
        for path in target_paths(app, env, options.desktop) {
            install(app, env, &cargo, &icon, &path)?;
            written.push(path.clone());
            entries.push(ManifestEntry {
                app: app.id.to_string(),
                path: path.to_string_lossy().into_owned(),
            });
        }
    }

    let manifest_path = manifest_path(env)?;
    manifest::save_manifest(
        &manifest_path,
        &Manifest {
            platform: env.platform.name().to_string(),
            entries,
        },
    )?;
    Ok(written)
}

/// Удалить перечисленные в манифесте ярлыки; вернуть число удалённых файлов.
pub fn remove_all(env: &Env) -> Result<usize, String> {
    let path = manifest_path(env)?;
    let manifest = manifest::load_manifest(&path)?;
    let mut removed = 0;
    for entry in &manifest.entries {
        let file = PathBuf::from(&entry.path);
        match std::fs::remove_file(&file) {
            Ok(()) => removed += 1,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!("не удалось удалить ярлык {}: {error}", file.display()));
            }
        }
    }
    std::fs::remove_file(&path)
        .map_err(|error| format!("не удалось удалить манифест ярлыков {}: {error}", path.display()))?;
    Ok(removed)
}

/// Путь к файлу ярлыка (или двум файлам при `--desktop`).
fn target_paths(app: &LaunchApp, env: &Env, desktop: bool) -> Vec<PathBuf> {
    let name = shortcut_name(app, env.platform);
    let mut paths = Vec::new();
    if let Some(dir) = primary_dir(env) {
        paths.push(dir.join(&name));
    }
    if desktop {
        if let Some(dir) = desktop_dir(env) {
            paths.push(dir.join(&name));
        }
    }
    paths
}

/// Имя файла ярлыка на текущей платформе.
fn shortcut_name(app: &LaunchApp, platform: Platform) -> String {
    match platform {
        Platform::Linux => format!("{}.desktop", app.title),
        Platform::Windows => format!("{}.lnk", app.title),
        Platform::MacOs => format!("{}.command", app.title),
    }
}

/// Основной каталог ярлыков.
fn primary_dir(env: &Env) -> Option<PathBuf> {
    match env.platform {
        Platform::Linux => linux::applications_dir(env.xdg_data_home.as_deref(), env.home.as_deref()),
        Platform::Windows => windows::start_menu_dir(env.appdata.as_deref()),
        Platform::MacOs => macos::applications_dir(env.home.as_deref()),
    }
}

/// Каталог рабочего стола (только для флага `--desktop`).
///
/// На всех трёх ОС рабочий стол — это `<домашний каталог>/Desktop`; на Linux
/// уважаем `XDG_DESKTOP_DIR`, потому что его переопределяют локализованные
/// системы (`~/Рабочий стол`).
fn desktop_dir(env: &Env) -> Option<PathBuf> {
    match env.platform {
        Platform::Linux => linux::desktop_dir(env.xdg_desktop.as_deref(), env.home.as_deref()),
        Platform::Windows | Platform::MacOs => env.home.clone().map(|home| home.join("Desktop")),
    }
}

/// Создать один ярлык по правилам платформы.
///
/// Linux и macOS получают готовый текстовый файл, Windows — сценарий PowerShell:
/// `.lnk` это бинарный формат, в него нельзя записать текст напрямую.
fn install(
    app: &LaunchApp,
    env: &Env,
    cargo: &CargoPath,
    icon: &Path,
    path: &Path,
) -> Result<(), String> {
    match env.platform {
        Platform::Linux => {
            let entry = linux::desktop_entry(app, env.crate_root, cargo, icon)?;
            linux::write_entry(path, &entry)
        }
        Platform::MacOs => {
            let script = macos::command_script(app, env.crate_root, cargo)?;
            write_script(path, &script)?;
            make_executable(path)
        }
        Platform::Windows => {
            let script = windows::shortcut_script(app, env.crate_root, cargo, path, icon)?;
            env.shell
                .run_powershell(&script)
                .map_err(|error| format!("не удалось создать ярлык {}: {error}", path.display()))
        }
    }
}

/// Записать текстовый файл ярлыка, создав каталоги.
fn write_script(path: &Path, content: &str) -> Result<(), String> {
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

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .map_err(|error| format!("нет доступа к {}: {error}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)
        .map_err(|error| format!("не удалось выставить права на {}: {error}", path.display()))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

/// Путь к манифесту созданных ярлыков.
fn manifest_path(env: &Env) -> Result<PathBuf, String> {
    let dir = manifest::data_dir(manifest::DataDirInputs {
        os: env.platform,
        xdg_data_home: env.xdg_data_home.clone(),
        home: env.home.clone(),
        appdata: env.appdata.clone(),
    })
    .ok_or_else(|| {
        "не удалось найти домашний каталог пользователя: задайте XDG_DATA_HOME, APPDATA или HOME"
            .to_string()
    })?;
    Ok(dir.join("shortcuts.json"))
}

/// Иконка для ярлыка: у генераторов своей нет, поэтому берём иконку раннера.
fn icon_path(crate_root: &Path) -> PathBuf {
    crate_root
        .join("crates/lidar-dataset-runner/icons/icon.png")
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    /// Подмена PowerShell: запоминает сценарии вместо выполнения.
    #[derive(Default)]
    pub struct FakeShell {
        scripts: RefCell<Vec<String>>,
        fail: bool,
    }

    impl FakeShell {
        /// Оболочка, которая всегда сообщает об ошибке.
        pub fn failing() -> Self {
            Self {
                scripts: RefCell::new(Vec::new()),
                fail: true,
            }
        }

        /// Все сценарии, которые попросили выполнить.
        pub fn scripts(&self) -> Vec<String> {
            self.scripts.borrow().clone()
        }
    }

    impl crate::shell::Shell for FakeShell {
        fn run_powershell(&self, the_script: &str) -> Result<(), String> {
            self.scripts.borrow_mut().push(the_script.to_string());
            if self.fail {
                Err("сценарий не выполнен".to_string())
            } else {
                Ok(())
            }
        }
    }

    fn repo_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn temp_env(name: &str) -> std::path::PathBuf {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parse_args_reads_all_flags() {
        let args: Vec<String> = ["--apps", "generators", "--desktop"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let options = super::parse_args(&args).unwrap();
        assert!(matches!(options.apps, crate::apps::Selector::Generators));
        assert!(options.desktop);
        assert!(!options.remove);
    }

    #[test]
    fn parse_args_rejects_unknown_flag() {
        let args: Vec<String> = vec!["--nope".to_string()];
        assert!(super::parse_args(&args).unwrap_err().contains("--nope"));
    }

    #[test]
    fn run_writes_desktop_files_and_manifest() {
        let root = repo_root();
        let data = temp_env("xtask-launch-data");
        let env = super::Env {
            crate_root: &root,
            platform: crate::manifest::Platform::Linux,
            xdg_data_home: Some(data.join("data")),
            home: Some(data.join("home")),
            appdata: None,
            xdg_desktop: None,
            cargo: Some("cargo".into()),
            shell: &FakeShell::default(),
        };
        let written = super::run(
            &super::LaunchOptions {
                apps: crate::apps::Selector::Generators,
                desktop: false,
                remove: false,
            },
            &env,
        )
        .unwrap();
        assert_eq!(written.len(), 5);
        for path in &written {
            assert!(path.is_file(), "{}", path.display());
            let text = std::fs::read_to_string(path).unwrap();
            assert!(text.contains("[Desktop Entry]"));
            assert!(text.contains("cargo") && text.contains("run -p"));
        }
        let manifest = crate::manifest::load_manifest(
            &data.join("data/lidar-dataset-generator/shortcuts.json"),
        )
        .unwrap();
        assert_eq!(manifest.entries.len(), 5);
        std::fs::remove_dir_all(&data).unwrap();
    }

    #[test]
    fn remove_fails_without_manifest() {
        let root = repo_root();
        let data = temp_env("xtask-remove-empty");
        let env = super::Env {
            crate_root: &root,
            platform: crate::manifest::Platform::Linux,
            xdg_data_home: Some(data.join("data")),
            home: Some(data.join("home")),
            appdata: None,
            xdg_desktop: None,
            cargo: None,
            shell: &FakeShell::default(),
        };
        let error = super::remove_all(&env).unwrap_err();
        assert!(error.contains("ярлык"), "{error}");
        std::fs::remove_dir_all(&data).unwrap();
    }

    #[test]
    fn remove_deletes_only_listed_files() {
        let root = repo_root();
        let data = temp_env("xtask-remove-listed");
        let env = super::Env {
            crate_root: &root,
            platform: crate::manifest::Platform::Linux,
            xdg_data_home: Some(data.join("data")),
            home: Some(data.join("home")),
            appdata: None,
            xdg_desktop: None,
            cargo: Some("cargo".into()),
            shell: &FakeShell::default(),
        };
        super::run(
            &super::LaunchOptions {
                apps: crate::apps::Selector::Runner,
                desktop: false,
                remove: false,
            },
            &env,
        )
        .unwrap();
        let bystander = data.join("data/applications/bystander.desktop");
        std::fs::write(&bystander, b"keep me").unwrap();

        let removed = super::remove_all(&env).unwrap();
        assert_eq!(removed, 1);
        assert!(bystander.is_file());
        std::fs::remove_dir_all(&data).unwrap();
    }

    #[test]
    fn windows_desktop_copy_lands_on_the_desktop() {
        let root = repo_root();
        let data = temp_env("xtask-launch-desktop-windows");
        let env = super::Env {
            crate_root: &root,
            platform: crate::manifest::Platform::Windows,
            xdg_data_home: None,
            home: Some(data.join("home")),
            appdata: Some(data.join("appdata")),
            xdg_desktop: None,
            cargo: Some("cargo".into()),
            shell: &FakeShell::default(),
        };
        let written = super::run(
            &super::LaunchOptions {
                apps: crate::apps::Selector::Runner,
                desktop: true,
                remove: false,
            },
            &env,
        )
        .unwrap();
        assert!(
            written
                .iter()
                .any(|path| path.ends_with("home/Desktop/LiDAR Dataset Runner.lnk")),
            "{:?}",
            written
        );
        std::fs::remove_dir_all(&data).unwrap();
    }

    #[test]
    fn macos_desktop_copy_lands_on_the_desktop() {
        let root = repo_root();
        let data = temp_env("xtask-launch-desktop-macos");
        let env = super::Env {
            crate_root: &root,
            platform: crate::manifest::Platform::MacOs,
            xdg_data_home: None,
            home: Some(data.join("home")),
            appdata: None,
            xdg_desktop: None,
            cargo: Some("cargo".into()),
            shell: &FakeShell::default(),
        };
        let written = super::run(
            &super::LaunchOptions {
                apps: crate::apps::Selector::Runner,
                desktop: true,
                remove: false,
            },
            &env,
        )
        .unwrap();
        assert!(
            written
                .iter()
                .any(|path| path.ends_with("home/Desktop/LiDAR Dataset Runner.command")),
            "{:?}",
            written
        );
        std::fs::remove_dir_all(&data).unwrap();
    }

    #[test]
    fn desktop_flag_copies_into_desktop_dir() {
        let root = repo_root();
        let data = temp_env("xtask-launch-desktop");
        let env = super::Env {
            crate_root: &root,
            platform: crate::manifest::Platform::Linux,
            xdg_data_home: Some(data.join("data")),
            home: Some(data.join("home")),
            appdata: None,
            xdg_desktop: Some(data.join("Desktop")),
            cargo: Some("cargo".into()),
            shell: &FakeShell::default(),
        };
        let written = super::run(
            &super::LaunchOptions {
                apps: crate::apps::Selector::Runner,
                desktop: true,
                remove: false,
            },
            &env,
        )
        .unwrap();
        assert_eq!(written.len(), 2);
        assert!(data.join("Desktop/LiDAR Dataset Runner.desktop").is_file());
        std::fs::remove_dir_all(&data).unwrap();
    }

    #[test]
    fn windows_shortcuts_go_through_powershell_with_their_own_path() {
        let root = repo_root();
        let data = temp_env("xtask-launch-windows-shell");
        let shell = FakeShell::default();
        let env = super::Env {
            crate_root: &root,
            platform: crate::manifest::Platform::Windows,
            xdg_data_home: None,
            home: Some(data.join("home")),
            appdata: Some(data.join("appdata")),
            xdg_desktop: None,
            cargo: Some("cargo".into()),
            shell: &shell,
        };
        super::run(
            &super::LaunchOptions {
                apps: crate::apps::Selector::Runner,
                desktop: true,
                remove: false,
            },
            &env,
        )
        .unwrap();

        let scripts = shell.scripts();
        assert_eq!(scripts.len(), 2, "{scripts:?}");
        assert!(
            scripts
                .iter()
                .all(|script| script.contains("WScript.Shell")),
            "{scripts:?}"
        );
        assert!(
            scripts
                .iter()
                .any(|script| script.contains("Start Menu") && script.contains(".lnk")),
            "{scripts:?}"
        );
        assert!(
            scripts
                .iter()
                .any(|script| script.contains("Desktop") && script.contains(".lnk")),
            "{scripts:?}"
        );
        std::fs::remove_dir_all(&data).unwrap();
    }

    #[test]
    fn windows_shell_failure_names_the_shortcut() {
        let root = repo_root();
        let data = temp_env("xtask-launch-windows-fail");
        let env = super::Env {
            crate_root: &root,
            platform: crate::manifest::Platform::Windows,
            xdg_data_home: None,
            home: Some(data.join("home")),
            appdata: Some(data.join("appdata")),
            xdg_desktop: None,
            cargo: Some("cargo".into()),
            shell: &FakeShell::failing(),
        };
        let error = super::run(
            &super::LaunchOptions {
                apps: crate::apps::Selector::Runner,
                desktop: false,
                remove: false,
            },
            &env,
        )
        .unwrap_err();
        assert!(error.contains("LiDAR Dataset Runner.lnk"), "{error}");
        std::fs::remove_dir_all(&data).unwrap();
    }

    #[test]
    fn posix_platforms_never_call_the_shell() {
        let root = repo_root();
        let data = temp_env("xtask-launch-no-shell");
        let shell = FakeShell::default();
        let env = super::Env {
            crate_root: &root,
            platform: crate::manifest::Platform::Linux,
            xdg_data_home: Some(data.join("data")),
            home: Some(data.join("home")),
            appdata: None,
            xdg_desktop: None,
            cargo: Some("cargo".into()),
            shell: &shell,
        };
        super::run(
            &super::LaunchOptions {
                apps: crate::apps::Selector::Runner,
                desktop: false,
                remove: false,
            },
            &env,
        )
        .unwrap();
        assert!(shell.scripts().is_empty());
        std::fs::remove_dir_all(&data).unwrap();
    }
}
