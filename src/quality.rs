//! Player-adjustable graphics quality. Resolution is left adaptive — the window
//! keeps the device's native DPR and the canvas auto-fits the screen — so this
//! setting controls *visual quality* (anti-aliasing), not resolution. The choice
//! persists across sessions (localStorage on web, a tmp file natively).

use bevy::prelude::*;

/// Render-quality tiers, mapped to MSAA (edge anti-aliasing). Resolution is no
/// longer tied to this — it stays adaptive to the device.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum QualityLevel {
    /// 流畅: no anti-aliasing — lightest, smoothest on weak GPUs.
    Saver,
    /// 标准: 2× MSAA — a balanced middle ground.
    Balanced,
    /// 精细: 4× MSAA — crispest edges, for flagships and desktop.
    Crisp,
}

impl QualityLevel {
    pub fn name(self) -> &'static str {
        match self {
            QualityLevel::Saver => "流畅",
            QualityLevel::Balanced => "标准",
            QualityLevel::Crisp => "精细",
        }
    }

    /// The MSAA level this tier maps to. WebGPU only *guarantees* sample counts 1
    /// and 4 (2× fails on many devices — "sample count (2) not supported"), so we
    /// only ever use Off or 4×.
    pub fn msaa(self) -> Msaa {
        match self {
            QualityLevel::Saver => Msaa::Off,
            QualityLevel::Balanced => Msaa::Off,
            QualityLevel::Crisp => Msaa::Sample4,
        }
    }

    pub fn next(self) -> QualityLevel {
        match self {
            QualityLevel::Saver => QualityLevel::Balanced,
            QualityLevel::Balanced => QualityLevel::Crisp,
            QualityLevel::Crisp => QualityLevel::Saver,
        }
    }

    fn tag(self) -> &'static str {
        match self {
            QualityLevel::Saver => "saver",
            QualityLevel::Balanced => "balanced",
            QualityLevel::Crisp => "crisp",
        }
    }

    fn from_tag(tag: &str) -> Option<QualityLevel> {
        match tag.trim() {
            "saver" => Some(QualityLevel::Saver),
            "balanced" => Some(QualityLevel::Balanced),
            "crisp" => Some(QualityLevel::Crisp),
            _ => None,
        }
    }
}

#[derive(Resource)]
pub struct GraphicsQuality {
    pub level: QualityLevel,
}

impl Default for GraphicsQuality {
    fn default() -> Self {
        Self::load()
    }
}

impl GraphicsQuality {
    /// Load the persisted tier, defaulting per platform: web (touch-heavy) starts
    /// balanced; native desktop starts crisp.
    pub fn load() -> Self {
        let fallback = if cfg!(target_arch = "wasm32") {
            QualityLevel::Balanced
        } else {
            QualityLevel::Crisp
        };
        let level = QualityLevel::from_tag(&load_quality()).unwrap_or(fallback);
        Self { level }
    }

    pub fn cycle(&mut self) {
        self.level = self.level.next();
        save_quality(self.level.tag());
    }
}

/// Apply the current quality (MSAA) to the camera whenever it changes (and once
/// at startup, since a freshly inserted resource counts as changed). Resolution
/// is deliberately left untouched so it stays adaptive to the device.
pub fn apply_quality(quality: Res<GraphicsQuality>, mut cameras: Query<&mut Msaa>) {
    if !quality.is_changed() {
        return;
    }
    let want = quality.level.msaa();
    for mut msaa in &mut cameras {
        if *msaa != want {
            *msaa = want;
        }
    }
}

// ---- persistence ----

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export function load_quality() {
  try { return globalThis.localStorage?.getItem('protect_carrot_quality') || ''; }
  catch (_) { return ''; }
}
export function save_quality(value) {
  try { globalThis.localStorage?.setItem('protect_carrot_quality', value); }
  catch (_) {}
}
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = load_quality)]
    fn load_quality_js() -> String;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = save_quality)]
    fn save_quality_js(value: &str);
}

#[cfg(target_arch = "wasm32")]
fn load_quality() -> String {
    load_quality_js()
}

#[cfg(target_arch = "wasm32")]
fn save_quality(value: &str) {
    save_quality_js(value);
}

#[cfg(not(target_arch = "wasm32"))]
fn load_quality() -> String {
    std::fs::read_to_string("tmp/quality.txt").unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn save_quality(value: &str) {
    let _ = std::fs::create_dir_all("tmp");
    let _ = std::fs::write("tmp/quality.txt", value);
}
