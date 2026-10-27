//! Presets for composite 100 x 100 m environments.
use landscapes_generator::LandscapePreset;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerrainPreset {
    Field,
    Forest,
    HillSlope,
    TerracedSlope,
    River,
    Pond,
    Coast,
    SlopingRiver,
    Waterfall,
    Village,
    Town,
    ForestCabin,
}

impl TerrainPreset {
    pub const ALL: [Self; 12] = [
        Self::Field,
        Self::Forest,
        Self::HillSlope,
        Self::TerracedSlope,
        Self::River,
        Self::Pond,
        Self::Coast,
        Self::SlopingRiver,
        Self::Waterfall,
        Self::Village,
        Self::Town,
        Self::ForestCabin,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Field => "field",
            Self::Forest => "forest",
            Self::HillSlope => "hill-slope",
            Self::TerracedSlope => "terraced-slope",
            Self::River => "river",
            Self::Pond => "pond",
            Self::Coast => "coast",
            Self::SlopingRiver => "sloping-river",
            Self::Waterfall => "waterfall",
            Self::Village => "village",
            Self::Town => "town",
            Self::ForestCabin => "forest-cabin",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "field" | "поле" => Some(Self::Field),
            "forest" | "лес" => Some(Self::Forest),
            "hill-slope" | "slope" | "склон" => Some(Self::HillSlope),
            "terraced-slope" | "terraces" | "террасы" => Some(Self::TerracedSlope),
            "river" | "река" | "речка" => Some(Self::River),
            "pond" | "пруд" => Some(Self::Pond),
            "coast" | "shore" | "побережье" => Some(Self::Coast),
            "sloping-river" | "наклонная-речка" => Some(Self::SlopingRiver),
            "waterfall" | "водопад" => Some(Self::Waterfall),
            "village" | "село" => Some(Self::Village),
            "town" | "городок" => Some(Self::Town),
            "forest-cabin" | "избушка" => Some(Self::ForestCabin),
            _ => None,
        }
    }

    pub const fn landscape(self) -> LandscapePreset {
        match self {
            Self::Field | Self::Forest | Self::Village | Self::Town | Self::ForestCabin => {
                LandscapePreset::Plain
            }
            Self::HillSlope => LandscapePreset::Slope,
            Self::TerracedSlope => LandscapePreset::MultiLevel,
            Self::River => LandscapePreset::River,
            Self::Pond => LandscapePreset::Pond,
            Self::Coast => LandscapePreset::Shore,
            Self::SlopingRiver => LandscapePreset::SlopingRiver,
            Self::Waterfall => LandscapePreset::Waterfalls,
        }
    }

    pub const fn amounts(self) -> (usize, usize, usize, usize) {
        // (trees, bushes, grass tufts, buildings) — target counts, not guaranteed.
        match self {
            Self::Field => (6, 16, 260, 0),
            Self::Forest => (35, 38, 200, 0),
            Self::HillSlope => (10, 18, 120, 0),
            Self::TerracedSlope => (14, 15, 100, 0),
            Self::River => (14, 24, 190, 0),
            Self::Pond => (12, 23, 190, 0),
            Self::Coast => (10, 18, 160, 0),
            Self::SlopingRiver => (12, 20, 125, 0),
            Self::Waterfall => (11, 18, 90, 0),
            Self::Village => (8, 14, 130, 8),
            Self::Town => (5, 9, 70, 14),
            Self::ForestCabin => (31, 30, 160, 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presets_are_complete_and_parseable() {
        assert_eq!(TerrainPreset::ALL.len(), 12);
        for preset in TerrainPreset::ALL {
            assert_eq!(TerrainPreset::from_name(preset.name()), Some(preset));
        }
    }
}
