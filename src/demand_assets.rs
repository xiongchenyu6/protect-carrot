//! Browser download preparation before leaving the menu. Native play stays local.
use crate::states::GameState;
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct PendingBattle {
    #[cfg(target_arch = "wasm32")]
    target: Option<GameState>,
    #[cfg(target_arch = "wasm32")]
    released: bool,
}

pub fn request(pending: &mut PendingBattle, next: &mut NextState<GameState>, target: GameState) {
    #[cfg(target_arch = "wasm32")]
    {
        pending.target = Some(target);
        pending.released = false;
        bridge_prepare();
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = pending;
        next.set(target);
    }
    #[cfg(target_arch = "wasm32")]
    let _ = next;
}

pub struct DemandAssetsPlugin;
impl Plugin for DemandAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingBattle>();
        #[cfg(target_arch = "wasm32")]
        app.add_systems(
            Update,
            (
                sync_viewport,
                finish,
                downloads_action,
                launcher_language.run_if(in_state(GameState::Menu)),
            ),
        )
        .add_systems(OnEnter(GameState::Menu), release);
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export function bridge_viewport() {
  const canvas=document.querySelector('canvas');
  if (!canvas) return [0,0,1];
  const rect=canvas.getBoundingClientRect();
  return [rect.width,rect.height,Math.round((devicePixelRatio||1)*1000)/1000];
}
export function bridge_language() { const l=window.carrotRequestedLanguage; window.carrotRequestedLanguage=null; return l==='en'?1:l==='zh-CN'?0:2; }
export function initial_touch_mode() { return matchMedia('(pointer: coarse)').matches; }
export function bridge_prepare() { window.carrotPrepareChapter(0); }
export function bridge_status() { const s=window.carrotChapterDownload?.status; return s==='ready'?1:s==='cancelled'?2:0; }
export function bridge_finish() { window.carrotAssetsReady?.(); window.carrotChapterDownload=null; }
export function bridge_release() { window.carrotReleaseChapter?.(); }
export function bridge_downloads() { window.carrotOpenDownloads?.(); }
"#)]
extern "C" {
    fn bridge_viewport() -> js_sys::Array;
    fn bridge_language() -> u32;
    pub fn initial_touch_mode() -> bool;
    fn bridge_prepare();
    fn bridge_status() -> u32;
    fn bridge_finish();
    fn bridge_release();
    fn bridge_downloads();
}

#[cfg(target_arch = "wasm32")]
fn sync_viewport(mut windows: Query<&mut Window>) {
    let dimensions = bridge_viewport();
    let width = dimensions.get(0).as_f64().unwrap_or(0.0);
    let height = dimensions.get(1).as_f64().unwrap_or(0.0);
    let scale = dimensions.get(2).as_f64().unwrap_or(1.0) as f32;
    if width <= 0.0 || height <= 0.0 || !scale.is_finite() || scale <= 0.0 {
        return;
    }
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    // Browser zoom/fullscreen can report a device-pixel content box inconsistent
    // with CSS coordinates. Keep rendering and Bevy pointer picking in one space.
    let physical_width = (width * scale as f64).round() as u32;
    let physical_height = (height * scale as f64).round() as u32;
    if window.resolution.scale_factor_override() != Some(scale) {
        window.resolution.set_scale_factor_override(Some(scale));
    }
    if window.physical_width() != physical_width || window.physical_height() != physical_height {
        window
            .resolution
            .set_physical_resolution(physical_width, physical_height);
    }
}

#[cfg(target_arch = "wasm32")]
fn launcher_language(
    mut language: ResMut<crate::i18n::Language>,
    mut dirty: ResMut<crate::ui::MenuDirty>,
) {
    let requested = match bridge_language() {
        0 => crate::i18n::Lang::Zh,
        1 => crate::i18n::Lang::En,
        _ => return,
    };
    if language.lang != requested {
        language.lang = requested;
        crate::i18n::set_current_lang(requested);
        dirty.0 = true;
    }
}

#[cfg(target_arch = "wasm32")]
fn release() {
    bridge_release();
}
#[cfg(target_arch = "wasm32")]
fn downloads_action(mut actions: MessageReader<crate::ui::UiActionActivated>) {
    for event in actions.read() {
        if matches!(event.action, crate::ui::UiAction::OpenDownloads) {
            bridge_downloads();
        }
    }
}
#[cfg(target_arch = "wasm32")]
fn finish(
    mut pending: ResMut<PendingBattle>,
    assets: Res<AssetServer>,
    effects: Res<crate::pixelfx::PixelFxLibrary>,
    mut next: ResMut<NextState<GameState>>,
) {
    let Some(target) = pending.target else {
        return;
    };
    match bridge_status() {
        2 => {
            pending.target = None;
        }
        1 => {
            if !pending.released {
                gate::release();
                pending.released = true;
            }
            if effects.ready(&assets) {
                pending.target = None;
                bridge_finish();
                next.set(target);
            }
        }
        _ => {}
    }
}

#[cfg(target_arch = "wasm32")]
pub mod gate {
    use std::{
        future::poll_fn,
        sync::{
            Mutex,
            atomic::{AtomicBool, Ordering},
        },
        task::{Poll, Waker},
    };
    static OPEN: AtomicBool = AtomicBool::new(false);
    static WAITERS: Mutex<Vec<Waker>> = Mutex::new(Vec::new());
    pub async fn wait(path: &std::path::Path) {
        if path == std::path::Path::new("audio/click.wav") {
            return;
        }
        if ![
            "audio",
            "pixelfx",
            "heroes_world",
            "heroes_walk",
            "paperdoll",
        ]
        .iter()
        .any(|root| path.starts_with(root))
        {
            return;
        }
        poll_fn(|cx| {
            let mut waiters = WAITERS.lock().unwrap();
            if OPEN.load(Ordering::Acquire) {
                Poll::Ready(())
            } else {
                if !waiters.iter().any(|w| w.will_wake(cx.waker())) {
                    waiters.push(cx.waker().clone());
                }
                Poll::Pending
            }
        })
        .await
    }
    pub fn release() {
        OPEN.store(true, Ordering::Release);
        for w in WAITERS.lock().unwrap().drain(..) {
            w.wake();
        }
    }
}
