//! Сцена растений: результат генерации с учётом режима.
//!
//! [`GenerationMode::Single`] — одно растение (дерево/куст) в центре или один
//! пучок травы; [`GenerationMode::Multiple`] — центральное растение плюс
//! похожие объекты того же пресета вокруг (для травы — поле пучков).
//! В обоих режимах соседние объекты получают дочерние seed-ы от глобального,
//! поэтому сцена целиком детерминирована от seed.

use bevy::math::{Vec2, Vec3};
use rand::RngExt;

use generation_core::occupancy::Occupancy;
use generation_core::Seed;

use super::config::{GenerationMode, PlantConfig, PlantType};
use super::grass::generate_field;
use super::grass::generate_tuft;
use super::grass::PlacedTuft;
use super::plant::Plant;

/// Растение, расставленное в сцене.
#[derive(Debug, Clone)]
pub struct PlacedPlant {
    /// Положение основания на карте (ось Z — вверх).
    pub position: Vec2,
    /// Поворот вокруг вертикали, радианы.
    pub rotation: f32,
    /// Само растение.
    pub plant: Plant,
}

/// Полная сгенерированная сцена растений.
#[derive(Debug, Clone)]
pub struct PlantScene {
    /// Режим генерации.
    pub mode: GenerationMode,
    /// Пресет.
    pub preset: super::config::PlantPreset,
    /// Global seed.
    pub seed: Seed,
    /// Расставленные деревья/кусты.
    pub plants: Vec<PlacedPlant>,
    /// Расставленные пучки травы.
    pub tufts: Vec<PlacedTuft>,
    /// Радиус сцены (для камеры), м.
    pub extent: f32,
    /// Максимальная высота объектов сцены, м.
    pub max_height: f32,
    /// Суммарное число точек облака.
    pub cloud_points: usize,
}

impl PlantScene {
    /// Сгенерировать сцену по конфигурации и режиму (детерминированно от seed).
    pub fn generate(config: &PlantConfig, mode: GenerationMode) -> Self {
        let mut scene = Self {
            mode,
            preset: config.preset,
            seed: config.seed,
            plants: Vec::new(),
            tufts: Vec::new(),
            extent: 0.0,
            max_height: 0.0,
            cloud_points: 0,
        };

        match config.preset.plant_type() {
            PlantType::Grass => scene.generate_grass(config, mode),
            PlantType::Tree | PlantType::Bush => scene.generate_woody(config, mode),
        }
        scene
    }

    /// Количество «объектов» сцены (растения + пучки).
    pub fn object_count(&self) -> usize {
        self.plants.len() + self.tufts.len()
    }

    /// Трава: один пучок или поле.
    fn generate_grass(&mut self, config: &PlantConfig, mode: GenerationMode) {
        let seed = self.seed;
        if mode == GenerationMode::Multiple {
            let tufts = generate_field(config, seed);
            for t in &tufts {
                self.extent = self
                    .extent
                    .max(t.position.length() + t.tuft.radius);
                self.max_height = self.max_height.max(t.tuft.height);
                self.cloud_points += t.tuft.point_cloud.len();
            }
            self.tufts = tufts;

            // Поле всегда чуть больше радиуса разлёта пучков.
            self.extent = self.extent.max(config.grass.field_radius);
        } else {
            let tuft = generate_tuft(config, seed.with_label("single"));
            self.extent = tuft.radius;
            self.max_height = tuft.height;
            self.cloud_points = tuft.point_cloud.len();
            self.tufts.push(PlacedTuft {
                position: Vec2::ZERO,
                rotation: 0.0,
                tuft,
            });
        }
    }

    /// Дерево/куст: центральное растение (+ соседи в множественном режиме).
    fn generate_woody(&mut self, config: &PlantConfig, mode: GenerationMode) {
        let seed = self.seed;

        // Центральное растение.
        let center = Plant::generate(config);
        self.push_plant(PlacedPlant {
            position: Vec2::ZERO,
            rotation: 0.0,
            plant: center,
        });

        if mode == GenerationMode::Single {
            return;
        }

        // Соседи того же пресета вокруг центра.
        let mut rng = seed.with_label("grove").rng();
        let count = rng.random_range(config.neighbors_min..=config.neighbors_max);
        // Радиус разлёта должен пережить большие кроны: при маленьком spread
        // «не пересечься с центром» и «поместиться в spread» несовместимы, и
        // соседи перестают ставиться вовсе.
        let spread = config
            .spread_radius
            .max(self.plants[0].plant.extent * 2.5);

        // Зоны уже занятых мест (включая центр).
        let mut areas: Vec<Occupancy> = self
            .plants
            .iter()
            .map(|p| Occupancy::circle(p.position, p.plant.extent * 0.9))
            .collect();

        for i in 0..count {
            // Сосед порождается один раз на место (в локальных координатах,
            // положение сцены на форму не влияет): попытки подобрать свободное
            // место с одной кроной вместо полной перегенерации дерева.
            let neighbor = Plant::generate(&config.with_seed(seed.child(i as u64 + 1)));
            let occ_r = neighbor.extent * 0.9;
            for _attempt in 0..24 {
                let angle = rng.random_range(0.0..std::f32::consts::TAU);
                let dist = rng.random_range(
                    self.plants[0].plant.extent * 1.05..=spread,
                );
                let position = Vec2::new(angle.cos() * dist, angle.sin() * dist);

                let occ = Occupancy::circle(position, occ_r);

                // Центр соседа — в пределах разлёта (сам сосед при больших
                // кронах может чуть выглядывать за круг), и не пересекаемся
                // с уже поставленными.
                let inside = position.length() <= spread;
                if inside && areas.iter().all(|a| !a.intersects(&occ)) {
                    areas.push(occ);
                    self.push_plant(PlacedPlant {
                        position,
                        rotation: rng.random_range(0.0..std::f32::consts::TAU),
                        plant: neighbor,
                    });
                    break;
                }
            }
        }
    }

    /// Учесть растение в метриках сцены.
    fn push_plant(&mut self, placed: PlacedPlant) {
        let p = &placed.plant;
        self.extent = self.extent.max(placed.position.length() + p.extent);
        self.max_height = self.max_height.max(p.height);
        self.cloud_points += p.point_cloud.len();
        self.plants.push(placed);
    }

    /// Верхняя точка сцены (для мира/камеры).
    pub fn top(&self) -> Vec3 {
        Vec3::new(0.0, 0.0, self.max_height)
    }
}
