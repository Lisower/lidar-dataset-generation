//! Конфигурация генерации растения — полностью детерминирована от seed.
//!
//! Пресеты растений обобщённые: любой пресет разворачивается в единую
//! [`PlantConfig`], которая описывает иерархию веток ([`PlantConfig::branches`]),
//! крону ([`FoliageConfig`]), траву ([`GrassConfig`]) и параметры расстановки.
//! При этом специфика типа растения (дерево/куст/трава) не теряется: она
//! видна в [`PlantPreset`], [`PlantType`] и в способе построения кроны.

use bevy::color::Color;
use bevy::ecs::resource::Resource;

use generation_core::Seed;

/// Тип растения — определяет, как именно строится объект и его крона.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum PlantType {
    /// Дерево: один заметный ствол и крона высоко над землёй.
    #[default]
    Tree,
    /// Куст: множество стволиков от земли, листва почти от самой земли.
    Bush,
    /// Трава: пучок травинок, объёмная визуальная модель и облако точек.
    Grass,
}

impl PlantType {
    /// Человекочитаемое имя.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Tree => "дерево",
            Self::Bush => "куст",
            Self::Grass => "трава",
        }
    }
}

/// Пресет растения (архетип).
///
/// Обобщённый список пресетов, охватывающий деревья, кусты и траву: каждый
/// пресет учитывает специфику своего типа (см. [`PlantPreset::plant_type`]),
/// но разворачивается в одну и ту же [`PlantConfig`].
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum PlantPreset {
    /// Лиственное дерево: массивная крона из листьев на концах веток.
    #[default]
    DeciduousTree,
    /// Ель: хвойное дерево с выраженной конической кроной.
    Spruce,
    /// Гибридная стилизация «Ель из старой программы»: очень высокая ель с
    /// ярусами лап по всему стволу (CSV `local/plants/old/Ель.csv`).
    Fir,
    /// Сосна: высокий ствол с голым низом и негустой кроной-«зонтиком».
    Pine,
    /// Ива: короткий толстый ствол, широкая раскидистая плакучая крона.
    Willow,
    /// Пальма: голый ствол и крона-«фонтан» из крупных листьев на верхушке.
    Palm,
    /// Пальма с кокосами: плотнее крона и короткие коричневые «кокосы»
    /// у основания кроны.
    PalmCoconuts,
    /// Куст: густая листва от самой земли.
    Bush,
    /// Можжевельник: низкий раскидистый хвойный кустарник с густой кроной.
    Juniper,
    /// Высокая густая трава (пышные пучки).
    TallGrass,
    /// Низкая редкая трава (жидкие пучки).
    SparseGrass,
}

impl PlantPreset {
    /// Все пресеты в порядке переключения.
    pub const ALL: [Self; 11] = [
        Self::DeciduousTree,
        Self::Spruce,
        Self::Fir,
        Self::Pine,
        Self::Willow,
        Self::Palm,
        Self::PalmCoconuts,
        Self::Bush,
        Self::Juniper,
        Self::TallGrass,
        Self::SparseGrass,
    ];

    /// Название для панели информации.
    pub const fn label(self) -> &'static str {
        match self {
            Self::DeciduousTree => "лиственное дерево",
            Self::Spruce => "ель",
            Self::Fir => "ель высокая",
            Self::Pine => "сосна",
            Self::Willow => "ива",
            Self::Palm => "пальма",
            Self::PalmCoconuts => "пальма с кокосами",
            Self::Bush => "куст",
            Self::Juniper => "можжевельник",
            Self::TallGrass => "высокая трава",
            Self::SparseGrass => "низкая трава",
        }
    }

    /// Тип растения, к которому относится пресет.
    pub const fn plant_type(self) -> PlantType {
        match self {
            Self::DeciduousTree
            | Self::Spruce
            | Self::Fir
            | Self::Pine
            | Self::Willow
            | Self::Palm
            | Self::PalmCoconuts => PlantType::Tree,
            Self::Bush | Self::Juniper => PlantType::Bush,
            Self::TallGrass | Self::SparseGrass => PlantType::Grass,
        }
    }

    /// Id пресета в каталоге `presets/plants` (id манифеста и имя папки
    /// пресета). Единый источник для загрузки из RON (`resolve` → каталог
    /// пресетов) и для записи пресетов (`examples/dump_presets.rs`).
    pub const fn ron_id(self) -> &'static str {
        match self {
            Self::DeciduousTree => "deciduous-tree",
            Self::Spruce => "spruce",
            Self::Fir => "fir",
            Self::Pine => "pine",
            Self::Willow => "willow",
            Self::Palm => "palm",
            Self::PalmCoconuts => "palm-coconuts",
            Self::Bush => "bush",
            Self::Juniper => "juniper",
            Self::TallGrass => "tall-grass",
            Self::SparseGrass => "sparse-grass",
        }
    }

    /// Следующий пресет по кругу.
    pub fn next(self) -> Self {
        match Self::ALL.iter().position(|&p| p == self) {
            Some(i) => Self::ALL[(i + 1) % Self::ALL.len()],
            None => Self::ALL[0],
        }
    }

    /// Разобрать пресет из имени (для CLI и тестов).
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "deciduous" | "дерево" | "лиственное" => Some(Self::DeciduousTree),
            "spruce" | "ель" => Some(Self::Spruce),
            "fir" | "ель высокая" | "высокая ель" => Some(Self::Fir),
            "pine" | "сосна" => Some(Self::Pine),
            "willow" | "ива" => Some(Self::Willow),
            "palm" | "пальма" => Some(Self::Palm),
            "palmcoconuts" | "пальма с кокосами" => Some(Self::PalmCoconuts),
            "bush" | "куст" => Some(Self::Bush),
            "juniper" | "можжевельник" => Some(Self::Juniper),
            "tallgrass" | "высокая трава" | "трава" => Some(Self::TallGrass),
            "sparsegrass" | "низкая трава" => Some(Self::SparseGrass),
            _ => None,
        }
    }
}

/// Режим генерации: количество объектов и их расстановка.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GenerationMode {
    /// Одиночный объект в центре сцены.
    #[default]
    Single,
    /// Центральный объект + похожие объекты того же пресета вокруг
    /// (для травы — поле пучков с плотностью пресета).
    Multiple,
}

impl GenerationMode {
    /// Противоположный режим.
    pub fn toggled(self) -> Self {
        match self {
            Self::Single => Self::Multiple,
            Self::Multiple => Self::Single,
        }
    }

    /// Короткое имя для панели информации.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Single => "одиночный",
            Self::Multiple => "множественный",
        }
    }
}

/// Способ построения кроны растения.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FoliageStyle {
    /// Плоские листья-ромбики на концах веток (дерево, куст).
    Leafy,
}

/// Куда навешивается крона.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FoliagePlacement {
    /// Только на концы терминальных веток уровня `min_level` и глубже.
    Terminal { min_level: usize },
}

/// Параметры кроны.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct FoliageConfig {
    /// Стиль кроны (листья).
    pub style: FoliageStyle,
    /// Куда навешивается крона.
    pub placement: FoliagePlacement,
    /// Основной цвет кроны.
    pub color: Color,
    /// Длина листа, м.
    pub length: f32,
    /// Ширина листа, м.
    pub width: f32,
    /// Сколько листьев на один узел кроны (терминальную ветку).
    pub per_anchor: usize,
    /// Сколько якорей по длине ветки (задел на будущие иглы).
    pub anchors: usize,
    /// Разброс наклона листа, градусы (объём кроны).
    pub pitch_jitter: f32,
    /// Сколько точек облака даёт один лист (1..=4).
    pub cloud_points: usize,
}

/// Параметры пучка травы.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct GrassConfig {
    /// Минимальная высота пучка, м.
    pub tuft_height_min: f32,
    /// Максимальная высота пучка, м.
    pub tuft_height_max: f32,
    /// Радиус пучка (разброс травинок по горизонтали), м.
    pub tuft_radius: f32,
    /// Минимальное число травинок в пучке.
    pub blades_min: usize,
    /// Максимальное число травинок в пучке.
    pub blades_max: usize,
    /// Ширина травинки у основания, м.
    pub blade_width: f32,
    /// Изгиб/наклон травинки (0 — строго вверх, ~1 — сильно клонится).
    pub blade_bend: f32,
    /// Цвет травы.
    pub color: Color,
    /// Сколько точек облака даёт весь пучок (не каждая травинка).
    pub tuft_cloud_points: usize,
    /// Радиус поля пучков (множественный режим), м.
    pub field_radius: f32,
    /// Минимальное число пучков в поле.
    pub tufts_min: usize,
    /// Максимальное число пучков в поле.
    pub tufts_max: usize,
}

/// Конфигурация одной ветки уровня (единица иерархии [`PlantConfig::branches`]).
///
/// Числа — диапазоны, из которых детерминированно от seed выбираются значения.
/// Семантика радиуса: для уровня 0 — абсолютные метры; для более глубоких
/// уровней — множитель к радиусу родительской ветки (после её сужения).
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct BranchGen {
    /// Уровень в иерархии дерева.
    pub level: usize,
    /// Номер ветки в уровне.
    pub branch_id: usize,

    // --- размеры (диапазоны) ---
    /// Минимальная длина, м.
    pub length_min: f32,
    /// Максимальная длина, м.
    pub length_max: f32,
    /// Минимальный нижний радиус (см. описание типа), м.
    pub radius_min: f32,
    /// Максимальный нижний радиус, м.
    pub radius_max: f32,
    /// Минимальный коэффициент сужения (верхний радиус = нижний * taper).
    pub taper_min: f32,
    /// Максимальный коэффициент сужения.
    pub taper_max: f32,

    // --- углы (градусы) ---
    /// Минимальное отклонение от вертикали.
    pub vertical_min: f32,
    /// Максимальное отклонение от вертикали.
    pub vertical_max: f32,
    /// Минимальный угол в горизонтальной плоскости.
    pub horizontal_min: f32,
    /// Максимальный угол в горизонтальной плоскости.
    pub horizontal_max: f32,

    // --- ветвление ---
    /// Минимальное число дочерних веток.
    pub children_min: usize,
    /// Максимальное число дочерних веток.
    pub children_max: usize,
    /// Номер ветки следующего уровня, с которого начинается окно кандидатов.
    /// Берутся все `children_max` веток уровня подряд — так у тройки шаблонов
    /// «продолжить/−поворот/+поворот» достижимы все три, и азимут не
    /// дрейфует в одну сторону (иначе крона закручивается спиралью).
    pub first_child_index: usize,

    /// Дети — строго подряд от `first_child_index` (первые `children` веток),
    /// как в старой программе. Нужно для деревьев из старых CSV: там первая
    /// ветка окна — «продолжение ствола/ветки», и случайный выбор из окна
    /// (см. `grow`) слишком часто её теряет, из-за чего дерево обрывается.
    /// Для пресетов нового движка (с окном-перемешиванием) — `false`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub consecutive_children: bool,

    /// Число граней меша сегмента ветки.
    pub mesh_sides: usize,

    /// Тапер — независимый множитель сегмента (как в старой программе:
    /// `minTaper..maxTaper` берётся как есть, не умножается на тапер родителя).
    /// Движок по умолчанию накапливает тапер (`taper × parent_taper`) — для
    /// старых CSV-деревьев с 30–40 уровнями это квадратично схлопывает радиус
    /// кроны до нуля. Для таких деревьев — `true`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub independent_taper: bool,

    /// Ограничение длины сегмента в метрах (после умножения на родителя).
    /// Нужно для старых CSV-деревьев, где конусы-«иглы» нижних уровней
    /// раздуваются до брёвен, а на верхушке превращаются в крошки — клэмп
    /// сглаживает уменьшение размера от уровня к уровню. `None` — без
    /// ограничения.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length_clamp: Option<(f32, f32)>,

    /// То же для нижнего радиуса сегмента (в метрах).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius_clamp: Option<(f32, f32)>,

    /// Собственный цвет сегмента (для старых CSV-деревьев, где ствол и крона
    /// окрашены по-разному). `None` — использовать [`PlantConfig::branch_color`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

/// Для `skip_serializing_if` (не сериализуем `consecutive_children: false`).
fn is_false(b: &bool) -> bool {
    !*b
}

impl BranchGen {
    /// Минимальная ветка уровня `level` с id `branch_id` (для аккуратных пресетов).
    pub const fn new(level: usize, branch_id: usize) -> Self {
        Self {
            level,
            branch_id,
            length_min: 0.4,
            length_max: 0.6,
            radius_min: 0.3,
            radius_max: 0.5,
            taper_min: 0.8,
            taper_max: 0.9,
            vertical_min: -20.0,
            vertical_max: 20.0,
            horizontal_min: -45.0,
            horizontal_max: 45.0,
            children_min: 2,
            children_max: 3,
            first_child_index: 0,
            mesh_sides: 8,
            consecutive_children: false,
            independent_taper: false,
            length_clamp: None,
            radius_clamp: None,
            color: None,
        }
    }
}

/// Иерархия конфигураций веток: `branches[level]` — список веток уровня.
pub type BranchLevels = Vec<Vec<BranchGen>>;

/// Полностью развёрнутая конфигурация генерации растения.
///
/// Пресеты хранятся как сериализованные [`PlantConfig`] в `config.ron`
/// (папки `presets/plants/...`): генератор читает файл и строит по нему
/// объект.
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlantConfig {
    /// Глобальный seed — вся случайность (включая ветки, крону, расстановку)
    /// детерминирована от него.
    pub seed: Seed,
    /// Пресет, из которого получена конфигурация.
    pub preset: PlantPreset,
    /// Тип растения.
    pub plant_type: PlantType,

    // --- ветки / ствол (дерево и куст) ---
    /// Иерархия конфигураций веток.
    pub branches: BranchLevels,
    /// Цвет веток (ствола).
    pub branch_color: Color,
    /// Максимальное число сегментов веток у одного растения (limit
    /// производительности: при достижении ветки больше не ветвятся).
    pub max_branches: usize,

    // --- крона ---
    /// Параметры кроны (дерево и куст).
    pub foliage: FoliageConfig,

    // --- трава ---
    /// Параметры пучков травы (тип `Grass`).
    pub grass: GrassConfig,

    // --- облако точек ---
    /// Шаг сетки облака точек на стволах/ветках, м.
    pub point_step: f32,
    /// Амплитуда шума сетки облака, м.
    pub jitter: f32,

    // --- расстановка в множественном режиме ---
    /// Радиус разлёта похожих объектов вокруг центрального, м.
    pub spread_radius: f32,
    /// Минимальное число объектов вокруг центрального (плюс сам центральный).
    pub neighbors_min: usize,
    /// Максимальное число объектов вокруг центрального.
    pub neighbors_max: usize,
}

impl PlantConfig {
    /// Копия конфигурации с другим seed (для соседних объектов множественного режима).
    pub fn with_seed(&self, seed: Seed) -> Self {
        let mut c = self.clone();
        c.seed = seed;
        c
    }
}
