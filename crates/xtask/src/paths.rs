//! Пути, от которых зависит создание ярлыков.
//!
//! Ярлык из меню приложений не наследует `PATH` из `.bashrc`, а rustup кладёт
//! `cargo` в `~/.cargo/bin`. Поэтому путь к cargo вычисляется при создании
//! ярлыка и записывается в него абсолютным.

use std::path::{Path, PathBuf};

/// Путь к `cargo`, который попадёт в ярлык.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoPath(pub String);

/// Найти cargo: переменная окружения, затем `PATH`, затем голое имя.
///
/// Голое имя — последний вариант: тогда ярлык работает в терминале, где `cargo`
/// доступен, и падает с понятной ошибкой там, где нет.
pub fn find_cargo(env_cargo: Option<&str>, path_var: Option<&str>) -> CargoPath {
    if let Some(cargo) = env_cargo.filter(|value| !value.is_empty()) {
        return CargoPath(cargo.to_string());
    }

    let binary = if cfg!(windows) { "cargo.exe" } else { "cargo" };
    if let Some(path_var) = path_var {
        for dir in std::env::split_paths(path_var) {
            let candidate = dir.join(binary);
            if candidate.is_file() {
                return CargoPath(candidate.to_string_lossy().into_owned());
            }
        }
    }

    CargoPath("cargo".to_string())
}

/// Каталог крейта приложения внутри репозитория.
pub fn crate_dir(crate_root: &Path, package: &str) -> PathBuf {
    crate_root.join("crates").join(package)
}

#[cfg(test)]
mod tests {
    #[test]
    fn find_cargo_prefers_env_then_path_then_bare_name() {
        assert_eq!(
            super::find_cargo(Some("/opt/cargo/bin/cargo"), Some("/usr/bin")).0,
            "/opt/cargo/bin/cargo"
        );
        assert_eq!(
            super::find_cargo(None, Some("/nonexistent-path-dir-12345")).0,
            "cargo"
        );

        let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/xtask-find-cargo");
        let bin = base.join("bin");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&bin).unwrap();
        let cargo = bin.join(if cfg!(windows) { "cargo.exe" } else { "cargo" });
        std::fs::write(&cargo, b"#!/bin/sh\n").unwrap();
        let found = super::find_cargo(None, Some(bin.to_str().unwrap())).0;
        assert_eq!(found, cargo.to_string_lossy());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn crate_dir_sits_under_crates() {
        assert_eq!(
            super::crate_dir(std::path::Path::new("/repo"), "plants-generator"),
            std::path::PathBuf::from("/repo/crates/plants-generator")
        );
    }
}
