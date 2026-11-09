//! Служебные команды репозитория.
//!
//! Приложения запускаются только из терминала, поэтому установщик ярлыков
//! выполняет роль «точки входа» для тех, кто не работает с `cargo` каждый день.
//! Аргументы разбираются руками, как в `terrains-generator`: новых зависимостей
//! ради одной команды не тянем.

mod apps;
mod launch;
mod linux;
mod macos;
mod manifest;
mod paths;
mod shell;
mod windows;

use manifest::Platform;

const HELP: &str = "\
xtask — служебные команды репозитория

Команды:
  launch-shortcuts [--apps runner|generators|all] [--desktop]
      создаёт ярлыки запуска приложений
  launch-shortcuts --remove
      удаляет ярлыки, созданные предыдущим вызовом

Флаги launch-shortcuts:
  --apps <набор>   runner — только раннер датасетов, generators — пять
                   Bevy-генераторов, all — все шесть (по умолчанию all)
  --desktop        дополнительно положить копию ярлыка на рабочий стол
  --remove         удалить ярлыки по манифесту xtask
  --help           эта справка

Ярлыки требуют Rust: первый запуск собирает зависимости и занимает несколько
минут. Запуск из меню не наследует PATH терминала, поэтому путь к cargo
вычисляется при создании ярлыка.
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = dispatch(&args) {
        eprintln!("xtask: {error}");
        std::process::exit(1);
    }
}

fn dispatch(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first() else {
        print!("{HELP}");
        return Ok(());
    };

    match command.as_str() {
        "--help" | "-h" | "help" => {
            print!("{HELP}");
            Ok(())
        }
        "launch-shortcuts" => {
            if args[1..].iter().any(|arg| arg == "--help" || arg == "-h") {
                print!("{HELP}");
                return Ok(());
            }
            let options = launch::parse_args(&args[1..])?;
            let env = environment();
            if options.remove {
                let removed = launch::remove_all(&env)?;
                println!("xtask: удалено ярлыков: {removed}");
            } else {
                let written = launch::run(&options, &env)?;
                for path in written {
                    println!("xtask: {}", path.display());
                }
            }
            Ok(())
        }
        other => Err(format!(
            "неизвестная команда: {other}; запустите xtask --help"
        )),
    }
}

/// Собрать окружение команды из переменных процесса.
fn environment() -> launch::Env<'static> {
    launch::Env {
        crate_root: crate_root(),
        platform: Platform::current(),
        xdg_data_home: env_path("XDG_DATA_HOME"),
        home: env_path("HOME").or_else(|| env_path("USERPROFILE")),
        appdata: env_path("APPDATA"),
        xdg_desktop: env_path("XDG_DESKTOP_DIR"),
        cargo: Some(resolve_cargo()),
        shell: &shell::SystemShell,
    }
}

/// Путь к cargo, который попадёт в ярлык.
///
/// Меню приложений не наследует `PATH` из `.bashrc`, а rustup кладёт `cargo` в
/// `~/.cargo/bin`: без абсолютного пути ярлык из меню просто не стартует.
fn resolve_cargo() -> String {
    let from_env = std::env::var("CARGO").ok();
    let path_var = std::env::var("PATH").ok();
    paths::find_cargo(from_env.as_deref(), path_var.as_deref()).0
}

/// Значение переменной окружения как путь.
fn env_path(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os(name).map(std::path::PathBuf::from)
}

/// Корень репозитория: `crates/xtask` → два уровня вверх.
///
/// Считается от расположения самого крейта, а не от cwd: ярлык можно создать из
/// любого каталога, и каталог крейта в ярлыке обязан быть правильным.
fn crate_root() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("каталог xtask всегда лежит внутри crates/")
}
