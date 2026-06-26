//! Monster bestiary: tracks how many of each monster species the player has slain.
//! The screen UI lives in `ui.rs` (reuses its widgets); this is just the data.

use crate::monster::{MONSTER_SPECIES, MonsterSpecies, resistance_summary, species_skill};
use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Resource)]
pub struct Bestiary {
    pub killed: HashMap<usize, u32>,
}

impl Default for Bestiary {
    fn default() -> Self {
        Bestiary {
            killed: load_bestiary_counts(),
        }
    }
}

impl Bestiary {
    pub fn record(&mut self, species_id: usize) -> bool {
        let entry = self.killed.entry(species_id).or_default();
        let first_seen = *entry == 0;
        *entry += 1;
        save_bestiary_counts(&self.killed);
        first_seen
    }
    pub fn count(&self, species_id: usize) -> u32 {
        self.killed.get(&species_id).copied().unwrap_or(0)
    }
}

fn encode_counts(killed: &HashMap<usize, u32>) -> String {
    let max_id = MONSTER_SPECIES.iter().map(|s| s.id).max().unwrap_or(0);
    (0..=max_id)
        .map(|id| killed.get(&id).copied().unwrap_or(0).to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn decode_counts(raw: &str) -> HashMap<usize, u32> {
    raw.split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|value| !value.is_empty())
        .enumerate()
        .filter_map(|(id, value)| {
            let count = value.parse::<u32>().unwrap_or(0);
            (count > 0).then_some((id, count))
        })
        .collect()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export function load_bestiary_counts() {
  try {
    return globalThis.localStorage?.getItem('protect_carrot_bestiary') || '';
  } catch (_) {
    return '';
  }
}
export function save_bestiary_counts(value) {
  try {
    globalThis.localStorage?.setItem('protect_carrot_bestiary', value);
  } catch (_) {}
}
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = load_bestiary_counts)]
    fn load_bestiary_counts_js() -> String;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = save_bestiary_counts)]
    fn save_bestiary_counts_js(value: &str);
}

#[cfg(target_arch = "wasm32")]
fn load_bestiary_counts() -> HashMap<usize, u32> {
    decode_counts(&load_bestiary_counts_js())
}

#[cfg(target_arch = "wasm32")]
fn save_bestiary_counts(killed: &HashMap<usize, u32>) {
    save_bestiary_counts_js(&encode_counts(killed));
}

#[cfg(not(target_arch = "wasm32"))]
fn load_bestiary_counts() -> HashMap<usize, u32> {
    std::fs::read_to_string("tmp/bestiary_counts.txt")
        .map(|raw| decode_counts(&raw))
        .unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn save_bestiary_counts(killed: &HashMap<usize, u32>) {
    let _ = std::fs::create_dir_all("tmp");
    let _ = std::fs::write("tmp/bestiary_counts.txt", encode_counts(killed));
}

/// One-line trait summary for a bestiary entry.
pub fn brief(species: &MonsterSpecies) -> String {
    let mut defense = Vec::new();
    if species.armor() > 0.0 || species.magic_resist() > 0.0 {
        defense.push(crate::i18n::tf(
            "甲{} 抗{}",
            &[
                &format!("{:.0}", species.armor()),
                &format!("{:.0}", species.magic_resist()),
            ],
        ));
    }
    defense.extend(resistance_summary(species.resist_profile()));
    let def = if defense.is_empty() {
        String::new()
    } else {
        format!("\n{}", defense.join(" "))
    };
    let (skill_name, skill_desc) = species_skill(species);
    crate::i18n::tf(
        "{}\nHP×{} 速×{}{}\n技能：{} {}\n{}",
        &[
            &crate::i18n::t(species.def().name),
            &format!("{:.1}", species.def().hp_mod * species.hp_mult),
            &format!("{:.1}", species.def().speed_mod * species.speed_mult),
            &def,
            &crate::i18n::t(skill_name),
            &crate::i18n::t(skill_desc),
            &species.traits(),
        ],
    )
}
