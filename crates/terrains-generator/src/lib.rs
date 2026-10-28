//! Terrain composition: 12 presets and collision-aware object placement.
pub mod placement;
pub mod preset;
pub mod scene;

pub use placement::{PlacedObject, PlacementMap, PlacementRules};
pub use preset::{TerrainPreset, TerrainPresetData};
pub use scene::{PlacedBuilding, RoadCorridor, TerrainScene, generate, generate_from_data};
