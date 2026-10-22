//! Seed-и и детерминированная случайность.
//!
//! Любая генерация в проекте полностью детерминирована от глобального `Seed`:
//! один и тот же seed всегда даёт один и тот же результат. Вышестоящие
//! генераторы пробрасывают свой seed нижестоящим; чтобы у каждого объекта
//! (здания, дерева, куска территории) была своя, но тоже детерминированная
//! случайность, от родительского seed выводятся дочерние через [`Seed::child`].

use rand::{rngs::StdRng, SeedableRng};
use serde::{Deserialize, Serialize};

use bevy::prelude::Resource;

/// Значение глобального seed по умолчанию.
pub const DEFAULT_GLOBAL_SEED: u64 = 0xC0FFEE;

/// Детерминированный seed генерации.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Seed(pub u64);

impl Seed {
    /// Новый seed из явного значения.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Случайный seed из системного источника энтропии.
    ///
    /// Используется только когда результат заранее не важен (например, при
    /// нажатии `R` на «совершенно новый вид»); всё остальное должно опираться
    /// на детерминированные seed-ы.
    pub fn from_entropy() -> Self {
        Self(rand::random())
    }

    /// Сырое значение seed.
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Дочерний seed по целочисленной метке (индексу объекта и т.п.).
    ///
    /// Детерминирован: `seed.child(a) == seed.child(a)` всегда, разные `a`
    /// дают практически всегда разные (и при этом непредсказуемые) значения.
    /// Нижние генераторы получают независимые потоки случайности.
    pub fn child(self, label: u64) -> Self {
        // SplitMix64: быстрый качественный хэш, не требует зависимостей.
        let mut z = self.0.wrapping_add(label.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        Self(z ^ (z >> 31))
    }

    /// Дочерний seed по строковой метке (названию элемента и т.п.).
    pub fn with_label(self, label: &str) -> Self {
        // FNV-1a по байтам метки.
        let mut h: u64 = 0xCBF2_9CE4_8422_2325;
        for b in label.bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
        self.child(h)
    }

    /// Генератор случайных чисел, полностью определяемый этим seed.
    pub fn rng(self) -> StdRng {
        StdRng::seed_from_u64(self.0)
    }
}

impl Default for Seed {
    fn default() -> Self {
        Self(DEFAULT_GLOBAL_SEED)
    }
}

impl From<u64> for Seed {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for Seed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}

/// Глобальный seed приложения/генератора как ресурс Bevy.
///
/// Пробрасывается вышестоящими генераторами нижестоящим и меняется при
/// перегенерации: `R` — новый случайный, `Shift + R` — тот же самый.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GlobalSeed(pub Seed);

impl GlobalSeed {
    /// Полностью новый случайный seed.
    pub fn regenerate(&mut self) {
        self.0 = Seed::from_entropy();
    }
}
