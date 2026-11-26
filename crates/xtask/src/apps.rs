//! Каталог приложений, для которых xtask пишет ярлыки.
//!
//! Список жёстко задан в коде: имя ярлыка раннера обязано совпадать с
//! `productName` из `tauri.conf.json`, иначе ярлык от xtask и ярлык из
//! установщика выглядят как два разных приложения.

/// Приложение, для которого создаётся ярлык.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchApp {
    /// Ключ для `--apps` и для манифеста.
    pub id: &'static str,
    /// Имя ярлыка.
    pub title: &'static str,
    /// Имя пакета cargo.
    pub package: &'static str,
    /// Описание в ярлыке.
    pub comment: &'static str,
}

/// Все приложения.
pub const APPS: &[LaunchApp] = &[
    LaunchApp {
        id: "runner",
        title: "LiDAR Dataset Runner",
        package: "lidar-dataset-runner",
        comment: "Требует Rust; первая сборка занимает несколько минут",
    },
    LaunchApp {
        id: "landscapes",
        title: "LiDAR Landscape Generator",
        package: "landscapes-generator",
        comment: "Требует Rust; первая сборка занимает несколько минут",
    },
    LaunchApp {
        id: "buildings",
        title: "LiDAR Building Generator",
        package: "buildings-generator",
        comment: "Требует Rust; первая сборка занимает несколько минут",
    },
    LaunchApp {
        id: "plants",
        title: "LiDAR Plant Generator",
        package: "plants-generator",
        comment: "Требует Rust; первая сборка занимает несколько минут",
    },
    LaunchApp {
        id: "terrains",
        title: "LiDAR Terrain Generator",
        package: "terrains-generator",
        comment: "Требует Rust; первая сборка занимает несколько минут",
    },
    LaunchApp {
        id: "simulator",
        title: "LiDAR Simulator",
        package: "lidar-simulator",
        comment: "Требует Rust; первая сборка занимает несколько минут",
    },
];

/// Какие приложения нужны.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selector {
    /// Только раннер датасетов.
    Runner,
    /// Пять Bevy-генераторов.
    Generators,
    /// Все шесть приложений.
    All,
}

impl Selector {
    /// Разобрать значение флага `--apps`.
    pub fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "runner" => Ok(Self::Runner),
            "generators" => Ok(Self::Generators),
            "all" => Ok(Self::All),
            other => Err(format!(
                "неизвестный набор приложений: {other}; ожидается runner, generators или all"
            )),
        }
    }
}

/// Приложения выбранного набора.
pub fn select_apps(selector: Selector) -> &'static [LaunchApp] {
    match selector {
        Selector::Runner => &APPS[..1],
        Selector::Generators => &APPS[1..],
        Selector::All => APPS,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_selector_returns_every_app() {
        assert_eq!(super::select_apps(super::Selector::All).len(), 6);
    }

    #[test]
    fn generators_selector_skips_the_runner() {
        let apps = super::select_apps(super::Selector::Generators);
        assert_eq!(apps.len(), 5);
        assert!(apps.iter().all(|app| app.id != "runner"));
    }

    #[test]
    fn runner_title_matches_product_name() {
        let runner = super::select_apps(super::Selector::Runner)[0];
        assert_eq!(runner.title, "LiDAR Dataset Runner");
        assert_eq!(runner.package, "lidar-dataset-runner");
    }

    #[test]
    fn every_app_has_unique_id_and_existing_crate() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ids: Vec<&str> = super::APPS.iter().map(|app| app.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), super::APPS.len());
        for app in super::APPS {
            assert!(
                root.join("crates").join(app.package).join("Cargo.toml").is_file(),
                "{}",
                app.package
            );
        }
    }
}
