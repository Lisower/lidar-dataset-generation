//! Рецепт генерации: что ставить на сцену и сколько.
//!
//! Рецепт — единственный вход запуска. Он сериализуется в `run.json` вместе с
//! результатом, чтобы датасет воспроизводился из собственных файлов, и
//! валидируется целиком до первой секунды генерации: неизвестный пресет,
//! несовпадение типа элемента и пресета или превышение лимитов — это ошибка
//! с текстом для интерфейса, а не пустая сцена.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::catalog::Catalog;

/// Сколько сцен допускается в одном запуске.
pub const MAX_SCENES: u32 = 1000;
/// Сколько объектов одного вида допускается на сцену.
pub const MAX_ELEMENT_COUNT: u32 = 2000;
/// Суммарный предел объектов на сцену (защита от «2000 зданий» на 100×100 м).
pub const MAX_TOTAL_COUNT: u32 = 5000;
/// Максимальный уклон под объектами, градусы.
pub const MAX_SLOPE_LIMIT: f32 = 45.0;
/// Сколько доступных id показывать в сообщении об ошибке.
const MAX_LISTED_IDS: usize = 10;

/// Что именно ставится на сцену.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ElementKind {
    /// Здание.
    Building,
    /// Дерево.
    Tree,
    /// Куст.
    Bush,
    /// Пучок травы.
    Grass,
}

impl ElementKind {
    /// Все виды элементов.
    pub const ALL: [Self; 4] = [Self::Building, Self::Tree, Self::Bush, Self::Grass];

    /// Ключ для статистики и UI (`building`, `tree`, `bush`, `grass`).
    pub const fn key(self) -> &'static str {
        match self {
            Self::Building => "building",
            Self::Tree => "tree",
            Self::Bush => "bush",
            Self::Grass => "grass",
        }
    }

    /// Категория каталога пресетов, из которой берётся пресет элемента.
    pub const fn category(self) -> &'static str {
        match self {
            Self::Building => "buildings",
            Self::Tree | Self::Bush | Self::Grass => "plants",
        }
    }

    /// Ожидаемый тип растения; `None` — для зданий тип не проверяется.
    pub const fn plant_type(self) -> Option<&'static str> {
        match self {
            Self::Building => None,
            Self::Tree => Some("tree"),
            Self::Bush => Some("bush"),
            Self::Grass => Some("grass"),
        }
    }
}

/// Один вид объектов на сцене.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementSpec {
    /// Вид элемента.
    pub kind: ElementKind,
    /// id пресета из соответствующей категории каталога.
    pub preset_id: String,
    /// Целевое количество объектов на сцену.
    pub count: u32,
}

/// Рецепт запуска.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recipe {
    /// Число сцен в батче.
    pub scenes: u32,
    /// Базовый сид запуска.
    pub seed: u64,
    /// id пресета ландшафта из каталога.
    pub landscape: String,
    /// Максимальный уклон под объектами, градусы.
    pub max_slope_deg: f32,
    /// Что и сколько ставить на сцену.
    pub elements: Vec<ElementSpec>,
}

impl Recipe {
    /// Запрошенное количество по видам элементов (`requested` из статистики).
    pub fn requested(&self) -> BTreeMap<String, u32> {
        let mut out = BTreeMap::new();
        for element in &self.elements {
            *out.entry(element.kind.key().to_string()).or_insert(0) += element.count;
        }
        out
    }

    /// Есть ли в рецепте здания (от них зависит сглаживание рельефа).
    pub fn has_buildings(&self) -> bool {
        self.elements
            .iter()
            .any(|e| e.kind == ElementKind::Building && e.count > 0)
    }
}

/// Одно замечание валидации: к какому полю формы оно относится.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    /// Поле рецепта в camelCase (`scenes`, `elements[0].presetId`, ...).
    pub field: String,
    /// Текст для пользователя.
    pub message: String,
}

impl ValidationIssue {
    /// Замечание по полю.
    fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

/// Проверить рецепт против каталога пресетов.
///
/// Возвращает пустой список, если рецепт можно запускать. Ничего не паникует:
/// пустой каталог даёт замечания по ландшафту и элементам.
pub fn validate(recipe: &Recipe, catalog: &Catalog) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    if !(1..=MAX_SCENES).contains(&recipe.scenes) {
        issues.push(ValidationIssue::new(
            "scenes",
            format!("число сцен должно быть от 1 до {MAX_SCENES}"),
        ));
    }

    if !recipe.max_slope_deg.is_finite()
        || !(1.0..=MAX_SLOPE_LIMIT).contains(&recipe.max_slope_deg)
    {
        issues.push(ValidationIssue::new(
            "maxSlopeDeg",
            format!("максимальный уклон должен быть от 1 до {MAX_SLOPE_LIMIT} градусов"),
        ));
    }

    if catalog.landscapes.is_empty() {
        issues.push(ValidationIssue::new(
            "landscape",
            "каталог пресетов пуст: пресеты ландшафтов не найдены",
        ));
    } else if !catalog.landscapes.iter().any(|p| p.id == recipe.landscape) {
        issues.push(ValidationIssue::new(
            "landscape",
            format!(
                "ландшафт «{}» не найден. Доступны: {}",
                recipe.landscape,
                listed_ids(catalog.landscapes.iter().map(|p| p.id.as_str()))
            ),
        ));
    }

    if recipe.elements.is_empty() {
        issues.push(ValidationIssue::new(
            "elements",
            "добавьте хотя бы один элемент на сцену",
        ));
    }

    let mut total = 0u32;
    for (index, element) in recipe.elements.iter().enumerate() {
        if element.count > MAX_ELEMENT_COUNT {
            issues.push(ValidationIssue::new(
                format!("elements[{index}].count"),
                format!(
                    "«{}»: не больше {MAX_ELEMENT_COUNT} объектов одного вида",
                    element.preset_id
                ),
            ));
        }
        total = total.saturating_add(element.count);
        check_preset(index, element, catalog, &mut issues);
    }

    if !recipe.elements.is_empty() && total == 0 {
        issues.push(ValidationIssue::new(
            "elements",
            "во всех элементах нулевое количество: сцена останется пустой",
        ));
    } else if total > MAX_TOTAL_COUNT {
        issues.push(ValidationIssue::new(
            "elements",
            format!("на сцену можно поставить не больше {MAX_TOTAL_COUNT} объектов"),
        ));
    }

    issues
}

/// Проверить, что пресет элемента существует и подходит его виду.
fn check_preset(
    index: usize,
    element: &ElementSpec,
    catalog: &Catalog,
    issues: &mut Vec<ValidationIssue>,
) {
    let field = format!("elements[{index}].presetId");
    let category = element.kind.category();
    let presets = catalog.category(category);

    if presets.is_empty() {
        let what = match element.kind {
            ElementKind::Building => "зданий",
            _ => "растений",
        };
        issues.push(ValidationIssue::new(
            field,
            format!("в каталоге нет пресетов {what}: «{}» выбрать нельзя", element.preset_id),
        ));
        return;
    }

    let Some(preset) = presets.iter().find(|p| p.id == element.preset_id) else {
        issues.push(ValidationIssue::new(
            field,
            format!(
                "пресет «{}» не найден. Доступны: {}",
                element.preset_id,
                listed_ids(presets.iter().map(|p| p.id.as_str()))
            ),
        ));
        return;
    };

    let Some(expected) = element.kind.plant_type() else {
        return;
    };
    match preset.plant_type.as_deref() {
        Some(actual) if actual == expected => {}
        Some(actual) => issues.push(ValidationIssue::new(
            field,
            format!(
                "пресет «{}» — это «{actual}», а элемент ждёт тип «{expected}». Подходящие: {}",
                element.preset_id,
                listed_ids(
                    presets
                        .iter()
                        .filter(|p| p.plant_type.as_deref() == Some(expected))
                        .map(|p| p.id.as_str())
                )
            ),
        )),
        None => issues.push(ValidationIssue::new(
            field,
            format!(
                "у пресета «{}» не читается config.ron: его нельзя использовать",
                element.preset_id
            ),
        )),
    }
}

/// Список доступных id, обрезанный до [`MAX_LISTED_IDS`].
fn listed_ids<'a>(ids: impl Iterator<Item = &'a str>) -> String {
    let ids: Vec<&str> = ids.collect();
    let shown: Vec<&str> = ids.iter().copied().take(MAX_LISTED_IDS).collect();
    let tail = if ids.len() > shown.len() {
        format!(" и ещё {}", ids.len() - shown.len())
    } else {
        String::new()
    };
    if shown.is_empty() {
        "нет".to_string()
    } else {
        format!("{}{tail}", shown.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::catalog::load_catalog;
    use crate::dataset::test_support::repo_presets;

    #[test]
    fn valid_recipe_has_no_issues() {
        let catalog = load_catalog(&repo_presets());
        let recipe = Recipe {
            scenes: 2,
            seed: 42,
            landscape: "plain".into(),
            max_slope_deg: 25.0,
            elements: vec![
                ElementSpec {
                    kind: ElementKind::Building,
                    preset_id: "rectangular".into(),
                    count: 2,
                },
                ElementSpec {
                    kind: ElementKind::Tree,
                    preset_id: "deciduous-tree".into(),
                    count: 5,
                },
                ElementSpec {
                    kind: ElementKind::Grass,
                    preset_id: "sparse-grass".into(),
                    count: 20,
                },
            ],
        };
        assert!(validate(&recipe, &catalog).is_empty());
    }

    #[test]
    fn grass_preset_rejected_for_tree_element() {
        let catalog = load_catalog(&repo_presets());
        let recipe = Recipe {
            scenes: 1,
            seed: 1,
            landscape: "plain".into(),
            max_slope_deg: 25.0,
            elements: vec![ElementSpec {
                kind: ElementKind::Tree,
                preset_id: "sparse-grass".into(),
                count: 1,
            }],
        };
        let issues = validate(&recipe, &catalog);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].field, "elements[0].presetId");
    }

    #[test]
    fn unknown_ids_and_limits_are_reported() {
        let catalog = load_catalog(&repo_presets());
        let recipe = Recipe {
            scenes: 0,
            seed: 0,
            landscape: "нет-такого".into(),
            max_slope_deg: 90.0,
            elements: vec![ElementSpec {
                kind: ElementKind::Building,
                preset_id: "нет-такого".into(),
                count: 5000,
            }],
        };
        let fields: Vec<_> = validate(&recipe, &catalog)
            .into_iter()
            .map(|i| i.field)
            .collect();
        assert!(fields.contains(&"scenes".to_string()));
        assert!(fields.contains(&"landscape".to_string()));
        assert!(fields.contains(&"maxSlopeDeg".to_string()));
        assert!(fields.contains(&"elements[0].presetId".to_string()));
        assert!(fields.contains(&"elements[0].count".to_string()));
    }

    #[test]
    fn empty_catalog_is_an_error_not_a_panic() {
        let catalog = Catalog::default();
        let recipe = Recipe {
            scenes: 1,
            seed: 0,
            landscape: "plain".into(),
            max_slope_deg: 20.0,
            elements: vec![ElementSpec {
                kind: ElementKind::Tree,
                preset_id: "bush".into(),
                count: 1,
            }],
        };
        let issues = validate(&recipe, &catalog);
        assert!(issues.iter().any(|i| i.field == "landscape"));
        assert!(issues.iter().any(|i| i.field == "elements[0].presetId"));
    }
}
