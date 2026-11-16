//! Data-driven terrain composition: reusable presets and collision-aware placement.
pub mod placement;
pub mod preset;
pub mod scene;
pub mod sources;

pub use placement::{PlacedObject, PlacementMap, PlacementRules};
pub use preset::{TerrainPreset, TerrainPresetData};
pub use scene::{
    PlacedBuilding, RoadCorridor, TerrainScene, generate, generate_from_data,
    try_generate_from_data,
};
