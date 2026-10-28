//! Пресеты для демонстрации: хранение, иерархический поиск и выбор.
//!
//! Каждый пресет — папка внутри `presets/<категория>/...`, в которой лежат:
//! - `manifest.ron` — общая информация (id, имя, описание);
//! - `config.ron` — генератор-специфичная конфигурация (её читает сам
//!   генератор и строит по ней объект).
//!
//! Папки вкладываются друг в друга. Папка с `manifest.ron` — пресет, если в
//! нём `is_group: false` (значение по умолчанию), либо группа-набор, если
//! `is_group: true`. Папка без манифеста — группа (обратная совместимость).
//! Группы могут вкладываться друг в друга.
//!
//! Программы распознают пресеты по содержимому манифеста (`id`, `kind`),
//! а не по имени папки: папки можно свободно переименовывать и перекладывать
//! по группам — приложение сразу увидит изменения (главное, чтобы RON были
//! оформлены правильно). Выбор пресетов настраивается в окне генератора
//! (Ctrl+S) и хранится как RON в `settings/<крейта>`.
//!
//! Слой чистых данных: здесь нет зависимостей от UI, только чтение каталога,
//! сериализация и случайный выбор.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use bevy::log::warn;
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

/// Прочитать конфигурацию пресета из его `config.ron`.
///
/// `T` — сериализуемый тип конфигурации генератора (напр. `LandscapeConfig`
/// или `PlantConfig`). `None` — файла нет или он не читается.
pub fn load_preset_config<T: serde::de::DeserializeOwned>(meta: &PresetMeta) -> Option<T> {
    let text = fs::read_to_string(&meta.config_path).ok()?;
    match ron::from_str::<T>(&text) {
        Ok(config) => Some(config),
        Err(err) => {
            warn!(
                "presets: не удалось прочитать config.ron {}: {err}",
                meta.config_path.display()
            );
            None
        }
    }
}

/// Относительный корень всех пресетов воркспейса (от корня проекта).
pub const PRESETS_DIR: &str = "presets";
/// Имя файла манифеста пресета.
pub const MANIFEST_FILE: &str = "manifest.ron";
/// Имя файла конфигурации пресета.
pub const CONFIG_FILE: &str = "config.ron";
/// Имя папки настроек крейта.
pub const SETTINGS_DIR: &str = "settings";
/// Имя файла выбранных пресетов.
pub const SETTINGS_FILE: &str = "preset-selection.ron";

/// Категории пресетов (подпапки `presets/`).
pub const CATEGORIES: [&str; 4] = ["landscapes", "buildings", "plants", "terrains"];

/// Манифест пресета или группы — общая информация, не зависящая от генератора.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetManifest {
    /// Уникальный id пресета в категории (обычно — путь к папке от корня
    /// категории, напр. `water/river`).
    pub id: String,
    /// Человекочитаемое имя для окна выбора и панели информации.
    pub name: String,
    /// Необязательное описание.
    #[serde(default)]
    pub description: String,
    /// Машиночитаемый тег («вид» пресета), напр. `tree`, `bush`, `grass`,
    /// `plain`, `cabin`. По нему программы находят пресет нужного вида, не
    /// завязываясь на имя папки. Пустая строка — тега нет. Пишется обычной
    /// строкой: `kind: "tree"` (в RON `Option` требует `Some(...)`, поэтому
    /// для удобства ручного редактирования используется строка).
    #[serde(default)]
    pub kind: String,
    /// `true` — папка является группой-набором (внутри другие группы/пресеты),
    /// а не самим пресетом. По умолчанию — `false`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_group: bool,
}

/// Вспомогательный сериализатор `skip_serializing_if` для `is_group`.
fn is_false(b: &bool) -> bool {
    !*b
}

/// RON-файл выбранных пресетов конкретного крейта (`settings/preset-selection.ron`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PresetSelectionFile {
    /// Id выбранных пресетов.
    #[serde(default)]
    pub selected: Vec<String>,
}

/// Один пресет в индексе: папка + прочитанный манифест.
#[derive(Debug, Clone)]
pub struct PresetMeta {
    /// Уникальный id в категории (из манифеста; по умолчанию — путь от корня
    /// категории с `/`).
    pub id: String,
    /// Имя для отображения.
    pub name: String,
    /// Описание.
    pub description: String,
    /// Машиночитаемый тег из манифеста (см. [`PresetManifest::kind`]).
    pub kind: String,
    /// Папка пресета (относительно корня категории вложенность).
    pub dir: PathBuf,
    /// Путь к `manifest.ron`.
    pub manifest_path: PathBuf,
    /// Путь к `config.ron`.
    pub config_path: PathBuf,
}

/// Группа пресетов (папка с `manifest.ron` и `is_group: true`, либо папка
/// без манифеста). Внутри — вложенные группы и пресеты.
#[derive(Debug, Clone)]
pub struct PresetGroup {
    /// Относительный путь папки от корня категории, напр. `water`.
    pub id: String,
    /// Имя папки (или имя из манифеста, если он есть).
    pub name: String,
    /// Вложенные группы.
    pub groups: Vec<PresetGroup>,
    /// Пресеты прямо в этой папке.
    pub presets: Vec<PresetMeta>,
}

impl PresetGroup {
    /// Все пресеты поддерева (включая вложенные группы).
    pub fn all_presets(&self) -> Vec<&PresetMeta> {
        let mut out: Vec<&PresetMeta> = self.presets.iter().collect();
        for group in &self.groups {
            out.extend(group.all_presets());
        }
        out
    }
}

/// Индекс пресетов одной категории (`landscapes`, `buildings`, `plants`,
/// `terrains`) с текущим выбором.
#[derive(Debug, Resource)]
pub struct PresetStore {
    /// Имя категории (подпапка `presets/`).
    pub category: String,
    /// Корень категории: `presets/<категория>`.
    pub root: PathBuf,
    /// Пресеты прямо в корне категории (обычно пусто — всё лежит в группах).
    pub root_presets: Vec<PresetMeta>,
    /// Дерево групп (вложенные группы и пресеты).
    pub groups: Vec<PresetGroup>,
    /// Плоский список всех пресетов (удобно для случайного выбора).
    pub presets: Vec<PresetMeta>,
    /// Множество выбранных id.
    pub selected: BTreeSet<String>,
}

impl PresetStore {
    /// Просканировать категорию `category` под `presets_root` (`presets/`).
    ///
    /// Не читает сохранённый выбор — его загружает [`PresetStore::load_selection`].
    pub fn discover(category: &str, presets_root: &Path) -> Self {
        let root = presets_root.join(category);
        let mut flat: Vec<PresetMeta> = Vec::new();
        let mut root_presets: Vec<PresetMeta> = Vec::new();
        let mut groups: Vec<PresetGroup> = Vec::new();

        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                match classify_dir(&path) {
                    DirKind::Preset(manifest) => {
                        if let Some(meta) = load_preset(&path, &root, manifest) {
                            flat.push(meta.clone());
                            root_presets.push(meta);
                        }
                    }
                    DirKind::Group(manifest) => {
                        groups.push(scan_group(&path, &root, &mut flat, manifest));
                    }
                    DirKind::Skip => {}
                }
            }
        }

        groups.sort_by(|a, b| a.name.cmp(&b.name));
        root_presets.sort_by(|a, b| a.name.cmp(&b.name));
        flat.sort_by(|a, b| a.name.cmp(&b.name));

        Self {
            category: category.to_string(),
            root,
            root_presets,
            groups,
            presets: flat,
            selected: BTreeSet::new(),
        }
    }

    /// Путь к RON-файлу выбранных пресетов крейта (`settings/preset-selection.ron`).
    pub fn settings_path(settings_dir: &Path) -> PathBuf {
        settings_dir.join(SETTINGS_FILE)
    }

    /// Загрузить выбор из `settings_dir`. Если файла нет или он не читается —
    /// выбрать все пресеты (поведение первого запуска).
    pub fn load_selection(&mut self, settings_dir: &Path) {
        self.selected.clear();
        let path = Self::settings_path(settings_dir);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => {
                self.select_all();
                return;
            }
        };
        let file = match ron::from_str::<PresetSelectionFile>(&text) {
            Ok(file) => file,
            Err(_) => {
                self.select_all();
                return;
            }
        };
        let known: Vec<&str> = self.presets.iter().map(|p| p.id.as_str()).collect();
        for id in file.selected {
            if known.contains(&id.as_str()) {
                self.selected.insert(id);
            }
        }
    }

    /// Сохранить текущий выбор в `settings_dir`.
    pub fn save_selection(&self, settings_dir: &Path) {
        let file = PresetSelectionFile {
            selected: self.selected.iter().cloned().collect(),
        };
        let pretty = ron::ser::to_string_pretty(&file, Default::default())
            .unwrap_or_else(|_| "()".to_string());
        if fs::create_dir_all(settings_dir).is_err() {
            return;
        }
        let _ = fs::write(Self::settings_path(settings_dir), pretty);
    }

    /// Выбран ли пресет `id`.
    pub fn is_selected(&self, id: &str) -> bool {
        self.selected.contains(id)
    }

    /// Добавить пресет в выбор. Возвращает `true`, если выбор изменился.
    pub fn select(&mut self, id: &str) -> bool {
        self.selected.insert(id.to_string())
    }

    /// Убрать пресет из выбора. Возвращает `true`, если выбор изменился.
    pub fn unselect(&mut self, id: &str) -> bool {
        self.selected.remove(id)
    }

    /// Переключить пресет. Возвращает `true`, если он теперь выбран.
    pub fn toggle(&mut self, id: &str) -> bool {
        if self.selected.remove(id) {
            false
        } else {
            self.selected.insert(id.to_string());
            true
        }
    }

    /// Все ли пресеты поддерева группы выбраны (для индикации чекбокса группы).
    pub fn group_all_selected(&self, group: &PresetGroup) -> bool {
        let all = group.all_presets();
        let Some(first) = all.first() else {
            return false;
        };
        self.selected.contains(&first.id)
            && all.iter().all(|p| self.selected.contains(&p.id))
    }

    /// Переключить группу: выбрать/снять всю группу целиком.
    /// Возвращает `true`, если группа теперь выбрана.
    pub fn toggle_group(&mut self, group: &PresetGroup) -> bool {
        let target = !self.group_all_selected(group);
        for preset in group.all_presets() {
            if target {
                self.selected.insert(preset.id.clone());
            } else {
                self.selected.remove(&preset.id);
            }
        }
        target
    }

    /// Выбрать все пресеты категории.
    pub fn select_all(&mut self) {
        self.selected = self.presets.iter().map(|p| p.id.clone()).collect();
    }

    /// Снять выбор со всех пресетов.
    pub fn clear(&mut self) {
        self.selected.clear();
    }

    /// Число выбранных пресетов.
    pub fn selected_count(&self) -> usize {
        self.selected.len()
    }

    /// Случайный выбранный пресет (истинно случайный, порядок не важен).
    /// `None` — если ничего не выбрано.
    pub fn random(&self) -> Option<&PresetMeta> {
        let count = self.selected.len();
        if count == 0 {
            return None;
        }
        let idx = rand::random::<u64>() % count as u64;
        self.presets
            .iter()
            .filter(|p| self.selected.contains(&p.id))
            .nth(idx as usize)
    }

    /// Выбрать указанный id (если он существует), остальной выбор сохранить.
    pub fn force_select(&mut self, id: &str) -> bool {
        let exists = self.presets.iter().any(|p| p.id == id);
        if exists {
            self.selected.insert(id.to_string());
        }
        exists
    }

    /// Найти метаданные пресета в этом индексе по id.
    pub fn meta(&self, id: &str) -> Option<&PresetMeta> {
        self.presets.iter().find(|p| p.id == id)
    }

    /// Найти первый пресет категории с указанным `kind` из манифеста
    /// (например `"tree"`, `"plain"`) — программы находят пресет нужного вида,
    /// не завязываясь на то, как названа его папка.
    pub fn by_kind(&self, kind: &str) -> Option<&PresetMeta> {
        self.presets.iter().find(|p| !p.kind.is_empty() && p.kind == kind)
    }
}

/// Найти пресет в *другой* категории по id (из манифеста) или по
/// относительному пути (`категория/путь/к/пресету`).
///
/// Сначала пробуем трактовать `id` как путь к папке, затем — полный обход
/// категории с поиском по `id` из манифеста. Так поиск не зависит от того,
/// в какой группе лежит пресет и как названа его папка: главное, чтобы манифест
/// был оформлен правильно.
pub fn resolve_cross_category(
    category: &str,
    id: &str,
    presets_root: &Path,
) -> Option<PresetMeta> {
    let category_root = presets_root.join(category);
    // 1) id как относительный путь к папке пресета.
    let direct = category_root.join(id);
    if let Some(meta) = load_preset_if_preset(&direct, &category_root) {
        return Some(meta);
    }
    // 2) Иначе — полный обход: ищем по id из манифеста либо по пути папки.
    scan_for_preset(&category_root, &category_root, id)
}

/// Загрузить пресет из папки, если это пресет (а не группа) и он читается.
fn load_preset_if_preset(dir: &Path, root: &Path) -> Option<PresetMeta> {
    match classify_dir(dir) {
        DirKind::Preset(manifest) => load_preset(dir, root, manifest),
        DirKind::Group(_) | DirKind::Skip => None,
    }
}

/// Рекурсивно обойти категорию и найти пресет, у которого id из манифеста
/// (или относительный путь папки) совпадает с `id`.
fn scan_for_preset(dir: &Path, root: &Path, id: &str) -> Option<PresetMeta> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        match classify_dir(&path) {
            DirKind::Preset(manifest) => {
                if let Some(meta) = load_preset(&path, root, manifest) {
                    if meta.id == id || rel_string(root, &path) == id {
                        return Some(meta);
                    }
                }
            }
            DirKind::Group(_) => {
                if let Some(found) = scan_for_preset(&path, root, id) {
                    return Some(found);
                }
            }
            DirKind::Skip => {}
        }
    }
    None
}

/// Тип папки: пресет, группа или не пресет вовсе.
enum DirKind {
    /// Пресет: есть `manifest.ron` с `is_group: false` (или без него).
    Preset(PresetManifest),
    /// Группа: `manifest.ron` c `is_group: true`, либо папка без манифеста.
    Group(Option<PresetManifest>),
    /// Папка с битым манифестом — пропускаем.
    Skip,
}

/// Классифицировать папку по содержимому `manifest.ron`.
fn classify_dir(dir: &Path) -> DirKind {
    let manifest_path = dir.join(MANIFEST_FILE);
    if !manifest_path.is_file() {
        return DirKind::Group(None);
    }
    let manifest = match fs::read_to_string(&manifest_path)
        .ok()
        .and_then(|text| ron::from_str::<PresetManifest>(&text).ok())
    {
        Some(manifest) => manifest,
        None => {
            warn!(
                "presets: не удалось прочитать манифест {}: пропускаю папку",
                manifest_path.display()
            );
            return DirKind::Skip;
        }
    };
    if manifest.is_group {
        DirKind::Group(Some(manifest))
    } else {
        DirKind::Preset(manifest)
    }
}

/// Папка — пресет, если в ней есть манифест без `is_group: true`.
pub fn is_preset_folder(dir: &Path) -> bool {
    matches!(classify_dir(dir), DirKind::Preset(_))
}

/// Человекочитаемый «id» по пути.
fn rel_string(root: &Path, dir: &Path) -> String {
    let rel = dir.strip_prefix(root).unwrap_or(dir);
    rel.to_string_lossy().replace('\\', "/")
}

/// Прочитать пресет из папки `dir` (файлы `manifest.ron` и `config.ron`).
/// Id берётся из манифеста; если он пуст — из пути от корня категории.
fn load_preset(dir: &Path, root: &Path, manifest: PresetManifest) -> Option<PresetMeta> {
    let manifest_path = dir.join(MANIFEST_FILE);
    let config_path = dir.join(CONFIG_FILE);
    if !config_path.is_file() {
        warn!(
            "presets: у пресета {} нет config.ron: пропускаю",
            manifest_path.display()
        );
        return None;
    }
    let default_id = rel_string(root, dir);
    let id = if manifest.id.trim().is_empty() {
        default_id
    } else {
        manifest.id
    };
    Some(PresetMeta {
        id,
        name: manifest.name,
        description: manifest.description,
        kind: manifest.kind,
        dir: dir.to_path_buf(),
        manifest_path,
        config_path,
    })
}

/// Рекурсивно просканировать группу: её непосредственные пресеты и подгруппы.
fn scan_group(
    dir: &Path,
    root: &Path,
    flat: &mut Vec<PresetMeta>,
    manifest: Option<PresetManifest>,
) -> PresetGroup {
    let fallback_name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let fallback_id = rel_string(root, dir);
    let (name, id) = match manifest {
        Some(m) => {
            let name = if m.name.trim().is_empty() {
                fallback_name
            } else {
                m.name
            };
            let id = if m.id.trim().is_empty() {
                fallback_id
            } else {
                m.id
            };
            (name, id)
        }
        None => (fallback_name, fallback_id),
    };
    let mut presets = Vec::new();
    let mut groups = Vec::new();
    let mut local = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            match classify_dir(&path) {
                DirKind::Preset(manifest) => {
                    if let Some(meta) = load_preset(&path, root, manifest) {
                        local.push(meta);
                    }
                }
                DirKind::Group(manifest) => {
                    groups.push(scan_group(&path, root, flat, manifest));
                }
                DirKind::Skip => {}
            }
        }
    }
    local.sort_by(|a, b| a.name.cmp(&b.name));
    groups.sort_by(|a, b| a.name.cmp(&b.name));
    flat.extend(local.iter().cloned());
    presets.extend(local);
    PresetGroup {
        id,
        name,
        groups,
        presets,
    }
}

/// Проверить, что категория — одна из известных подпапок `presets/`.
pub fn is_known_category(category: &str) -> bool {
    CATEGORIES.contains(&category)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Временная папка теста — в `target/` воркспейса (системный TEMP в этой
    /// среде может быть недоступен для записи).
    fn tmp_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("preset-tests")
            .join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Создать файлы группы/пресета `rel` (манифест + `config.ron`).
    fn write_entry(
        root: &Path,
        rel: &str,
        id: &str,
        name: &str,
        kind: Option<&str>,
        is_group: bool,
    ) {
        let dir = root.join(rel);
        fs::create_dir_all(&dir).unwrap();
        let kind = kind.map(|k| format!("    kind: \"{k}\",\n")).unwrap_or_default();
        let group = if is_group { "    is_group: true,\n" } else { "" };
        let manifest =
            format!("(\n    id: \"{id}\",\n    name: \"{name}\",\n{kind}{group})\n");
        fs::write(dir.join(MANIFEST_FILE), manifest).unwrap();
        // Достаточно факта существования файла — содержимое при discovery не
        // парсится.
        fs::write(dir.join(CONFIG_FILE), "(seed: 0)").unwrap();
    }

    #[test]
    fn discovers_groups_with_and_without_manifest() {
        let root = tmp_dir("discover");
        let presets = root.join("presets");
        // Группа с манифестом (is_group: true) + вложенный пресет.
        write_entry(&presets, "plants/high", "high", "Высокие", None, true);
        write_entry(
            &presets,
            "plants/high/deciduous-tree",
            "deciduous-tree",
            "Лиственное дерево",
            Some("tree"),
            false,
        );
        // Группа без манифеста (легаси) + вложенный пресет.
        write_entry(
            &presets,
            "plants/low/tall-grass",
            "tall-grass",
            "Высокая трава",
            Some("grass"),
            false,
        );
        // Пресет прямо в корне категории.
        write_entry(&presets, "plants/root-plant", "root-plant", "Корневое", None, false);

        let store = PresetStore::discover("plants", &presets);

        assert_eq!(store.presets.len(), 3);
        assert_eq!(store.root_presets.len(), 1);
        assert_eq!(store.root_presets[0].id, "root-plant");

        assert_eq!(store.groups.len(), 2);
        // Имя группы из манифеста.
        let high = store.groups.iter().find(|g| g.id == "high").unwrap();
        assert_eq!(high.name, "Высокие");
        assert_eq!(high.presets.len(), 1);
        assert_eq!(high.presets[0].id, "deciduous-tree");
        // Группа без манифеста — имя из имени папки.
        let low = store.groups.iter().find(|g| g.id == "low").unwrap();
        assert_eq!(low.name, "low");

        assert_eq!(store.presets.len(), 3);
    }

    #[test]
    fn by_kind_finds_presets_across_groups() {
        let root = tmp_dir("by_kind");
        let presets = root.join("presets");
        write_entry(&presets, "plants/high/deciduous-tree", "deciduous-tree", "Дерево", Some("tree"), false);
        write_entry(&presets, "plants/high/spruce", "spruce", "Ель", Some("tree"), false);
        write_entry(&presets, "plants/medium/bush", "bush", "Куст", Some("bush"), false);

        let store = PresetStore::discover("plants", &presets);
        assert_eq!(store.by_kind("tree").map(|m| m.id.as_str()), Some("deciduous-tree"));
        assert_eq!(store.by_kind("bush").map(|m| m.id.as_str()), Some("bush"));
        assert!(store.by_kind("missing").is_none());
    }

    #[test]
    fn cross_category_resolves_by_manifest_id_in_groups() {
        let root = tmp_dir("cross");
        let presets = root.join("presets");
        // Пресет лежит НЕ по пути `plants/deciduous-tree`, а внутри группы.
        write_entry(&presets, "plants/trees/deciduous-tree", "deciduous-tree", "Дерево", Some("tree"), false);
        write_entry(&presets, "plants/low/tall-grass", "tall-grass", "Трава", None, false);

        let meta = resolve_cross_category("plants", "deciduous-tree", &presets)
            .expect("пресет должен найтись по id манифеста");
        assert_eq!(meta.id, "deciduous-tree");
        assert!(meta.dir.ends_with("trees/deciduous-tree"), "{}", meta.dir.display());

        // По пути тоже работает.
        let by_path = resolve_cross_category("plants", "low/tall-grass", &presets)
            .expect("пресет должен найтись по пути");
        assert_eq!(by_path.id, "tall-grass");
    }

    #[test]
    fn old_style_manifest_still_parses() {
        // Манифесты без kind/is_group (как писали раньше) должны читаться.
        let text = "(\n    id: \"plain\",\n    name: \"равнина\",\n    description: \"\",\n)\n";
        let manifest: PresetManifest = ron::from_str(text).unwrap();
        assert_eq!(manifest.id, "plain");
        assert_eq!(manifest.kind, "");
        assert!(!manifest.is_group);

        let text = "(\n    id: \"g\",\n    name: \"Группа\",\n    is_group: true,\n    kind: \"set\",\n)\n";
        let manifest: PresetManifest = ron::from_str(text).unwrap();
        assert!(manifest.is_group);
        assert_eq!(manifest.kind, "set");
    }

    #[test]
    #[ignore = "manual: требует сгенерированного дерева presets/"]
    fn real_tree_cross_category_resolution() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("presets");
        // Растения лежат в группах high/medium/low, но ищутся по id манифеста.
        for (category, id) in [
            ("plants", "deciduous-tree"),
            ("plants", "bush"),
            ("plants", "tall-grass"),
            ("landscapes", "plain"),
            ("buildings", "cottage"),
        ] {
            let meta = resolve_cross_category(category, id, &root)
                .unwrap_or_else(|| panic!("не нашёл {category}/{id} в {}", root.display()));
            assert_eq!(meta.id, id);
        }
    }
}
