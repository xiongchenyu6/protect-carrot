//! Bevy Fluent bridge. The existing `i18n` module remains the production string
//! path today; this module loads real Fluent bundles now so future migration can
//! happen incrementally instead of introducing localization plumbing later.

use crate::i18n::{Lang, Language};
use bevy::prelude::*;
use bevy_fluent::exts::fluent::BundleExt;
use bevy_fluent::{BundleAsset, FluentPlugin, Locale, Localization};

pub const BUNDLE_PATHS: [&str; 2] = ["locales/zh-CN/main.ftl.ron", "locales/en-US/main.ftl.ron"];

#[derive(Resource)]
pub struct FluentBundles {
    handles: Vec<Handle<BundleAsset>>,
    built_for: Option<Lang>,
}

#[derive(Resource, Default)]
pub struct FluentStatus {
    loaded: bool,
    active_locales: Vec<String>,
}

impl FluentStatus {
    pub fn summary(&self) -> String {
        if self.loaded {
            format!("loaded [{}]", self.active_locales.join(" -> "))
        } else {
            "loading".to_string()
        }
    }
}

pub fn fluent_plugin() -> FluentPlugin {
    FluentPlugin
}

pub fn locale_for(lang: Lang) -> Locale {
    let requested = match lang {
        Lang::Zh => "zh-CN",
        Lang::En => "en-US",
    };
    Locale::new(requested.parse().expect("valid Fluent locale"))
        .with_default("en-US".parse().expect("valid Fluent default locale"))
}

pub fn load_fluent_bundles(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(FluentBundles {
        handles: BUNDLE_PATHS.iter().map(|path| assets.load(*path)).collect(),
        built_for: None,
    });
}

pub fn sync_fluent_locale(lang: Res<Language>, mut locale: ResMut<Locale>) {
    if lang.is_changed() {
        *locale = locale_for(lang.lang);
    }
}

pub fn build_fluent_localization(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    bundles: Option<ResMut<FluentBundles>>,
    bundle_assets: Res<Assets<BundleAsset>>,
    locale: Res<Locale>,
    lang: Res<Language>,
    mut status: ResMut<FluentStatus>,
) {
    let Some(mut bundles) = bundles else {
        return;
    };
    if bundles.built_for == Some(lang.lang) {
        return;
    }
    if bundles
        .handles
        .iter()
        .any(|handle| !asset_server.is_loaded(handle.id()))
    {
        status.loaded = false;
        return;
    }

    let entries: Vec<_> = bundles
        .handles
        .iter()
        .filter_map(|handle| bundle_assets.get(handle).map(|asset| (handle, asset)))
        .collect();
    if entries.len() != bundles.handles.len() {
        status.loaded = false;
        return;
    }

    let mut localization = Localization::new();
    for wanted in locale.fallback_chain(entries.iter().map(|(_, asset)| asset.locale())) {
        if let Some((handle, asset)) = entries
            .iter()
            .find(|(_, asset)| asset.locale() == wanted)
            .copied()
        {
            localization.insert(handle, asset);
        }
    }

    status.active_locales = localization
        .locales()
        .map(|locale| locale.to_string())
        .collect();
    status.loaded = !status.active_locales.is_empty();
    bundles.built_for = Some(lang.lang);
    commands.insert_resource(localization);
}
