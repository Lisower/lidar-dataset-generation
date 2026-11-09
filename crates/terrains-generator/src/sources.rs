//! Weighted references into the existing PresetStore; no geometry lives here.
use std::collections::BTreeMap;
use std::path::Path;

use generation_core::presets::{PresetGroup, PresetMeta, PresetStore};
use rand::RngExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresetSource {
    pub id: String,
    #[serde(default)]
    pub group: bool,
    #[serde(default = "one")]
    pub weight: f32,
    #[serde(default)]
    pub min_water_distance: f32,
    #[serde(default)]
    pub max_water_distance: Option<f32>,
}

fn one() -> f32 {
    1.0
}

impl PresetSource {
    pub fn preset(id: &str) -> Self {
        Self {
            id: id.into(),
            group: false,
            weight: 1.0,
            min_water_distance: 0.0,
            max_water_distance: None,
        }
    }
    pub fn group(id: &str) -> Self {
        Self {
            group: true,
            ..Self::preset(id)
        }
    }
    pub fn accepts_water_distance(&self, distance: f32) -> bool {
        distance >= self.min_water_distance
            && self.max_water_distance.is_none_or(|max| distance <= max)
    }
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty()
            || !self.weight.is_finite()
            || self.weight <= 0.0
            || !self.min_water_distance.is_finite()
            || self.min_water_distance < 0.0
            || self
                .max_water_distance
                .is_some_and(|max| !max.is_finite() || max < self.min_water_distance)
        {
            return Err(format!("Некорректный источник пресетов: {self:?}"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub source: PresetSource,
    pub meta: Option<PresetMeta>,
}

fn groups<'a>(list: &'a [PresetGroup], id: &str, root: &Path, out: &mut Vec<&'a PresetGroup>) {
    for group in list {
        let path = group
            .dir
            .strip_prefix(root)
            .unwrap_or(&group.dir)
            .to_string_lossy()
            .replace('\\', "/");
        if group.id == id || path == id {
            out.push(group);
        }
        groups(&group.groups, id, root, out);
    }
}

/// New lists override legacy single ids. Group weight is shared among its leaves.
/// Stable id ordering and deduplication make filesystem traversal order irrelevant.
pub fn resolve(
    category: &str,
    sources: &[PresetSource],
    legacy: &str,
    root: Option<&Path>,
) -> Result<Vec<Candidate>, String> {
    let refs = if sources.is_empty() {
        if legacy.is_empty() {
            Vec::new()
        } else {
            vec![PresetSource::preset(legacy)]
        }
    } else {
        sources.to_vec()
    };
    let store = root.map(|root| PresetStore::discover(category, root));
    let mut found: BTreeMap<String, Candidate> = BTreeMap::new();
    for source in refs {
        source.validate()?;
        let leaves = match &store {
            Some(store) if source.group => {
                let mut matches = Vec::new();
                groups(&store.groups, &source.id, &store.root, &mut matches);
                if matches.len() != 1 {
                    return Err(format!(
                        "{category}: группа '{}' отсутствует или неоднозначна",
                        source.id
                    ));
                }
                let leaves = matches[0].all_presets();
                if leaves.is_empty() {
                    return Err(format!("{category}: пустая группа '{}'", source.id));
                }
                leaves.into_iter().cloned().map(Some).collect::<Vec<_>>()
            }
            Some(store) => {
                let matches: Vec<_> = store
                    .presets
                    .iter()
                    .filter(|meta| {
                        meta.id == source.id
                            || meta.dir.strip_prefix(&store.root).is_ok_and(|path| {
                                path.to_string_lossy().replace('\\', "/") == source.id
                            })
                    })
                    .collect();
                if matches.len() != 1 {
                    return Err(format!(
                        "{category}: пресет '{}' отсутствует или неоднозначен",
                        source.id
                    ));
                }
                vec![Some(matches[0].clone())]
            }
            None if source.group => {
                return Err(format!("Для группы '{}' нужен каталог presets", source.id));
            }
            None => vec![None],
        };
        let weight = source.weight / leaves.len() as f32;
        for meta in leaves {
            let id = meta
                .as_ref()
                .map_or_else(|| source.id.clone(), |meta| meta.id.clone());
            let mut item = source.clone();
            item.id = id.clone();
            item.group = false;
            item.weight = weight;
            if let Some(existing) = found.get(&id) {
                if existing.meta.as_ref().map(|m| &m.config_path)
                    != meta.as_ref().map(|m| &m.config_path)
                {
                    return Err(format!(
                        "{category}: id '{id}' используется разными пресетами"
                    ));
                }
            }
            found.entry(id).or_insert(Candidate { source: item, meta });
        }
    }
    Ok(found.into_values().collect())
}

pub fn choose<T>(items: &[T], weight: impl Fn(&T) -> f32, rng: &mut impl rand::Rng) -> usize {
    let total: f64 = items.iter().map(|item| weight(item) as f64).sum();
    let mut pick = rng.random::<f64>() * total;
    for (index, item) in items.iter().enumerate() {
        pick -= weight(item) as f64;
        if pick < 0.0 {
            return index;
        }
    }
    items.len() - 1
}
