//! Vendored fork of `paperdoll-tar` 0.1.1.
//!
//! Upstream `read()` unpacks the tar archive into `TempDir::new()` →
//! `std::env::temp_dir()`, which PANICS on wasm ("no filesystem on this
//! platform") and killed the web build at startup. This fork reads the whole
//! archive **in memory** — no filesystem at all — so the same code path works
//! on native and wasm. `save()` keeps the upstream tempdir-free rewrite too
//! (streams straight from memory into the tar builder).

use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
pub use paperdoll;
use paperdoll::{Manifest, PaperdollFactory};
use tar::{Archive, Builder, Header};

/// The file extension.
pub const EXTENSION_NAME: &'static str = "ppd";

/// The file name of the manifest file saved in the `ppd` file.
pub const FILE_NAME_MANIFEST: &'static str = "manifest.yml";

/// Loads a paperdoll project from the path of a `ppd` file.
pub fn load<P>(path: P) -> Result<PaperdollFactory>
where
    P: AsRef<Path>,
{
    read(std::fs::File::open(&path)?)
}

/// Reads a paperdoll project from a reader containing the bytes of a `ppd`
/// file. Fully in-memory: works on wasm.
pub fn read<R>(r: R) -> Result<PaperdollFactory>
where
    R: Read,
{
    let mut archive = Archive::new(r);

    let mut files: HashMap<PathBuf, Vec<u8>> = HashMap::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path()?.into_owned();
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        files.insert(path, buf);
    }

    let manifest_bytes = files
        .get(Path::new(FILE_NAME_MANIFEST))
        .ok_or_else(|| anyhow!("ppd archive is missing {FILE_NAME_MANIFEST}"))?;
    let mut manifest: Manifest = serde_yaml::from_slice(manifest_bytes)?;

    for doll in &mut manifest.dolls {
        if doll.path.is_empty() {
            continue;
        }
        let bytes = files
            .get(Path::new(&doll.path))
            .ok_or_else(|| anyhow!("ppd archive is missing doll image {}", doll.path))?;
        let img = image::load_from_memory(bytes)?.into_rgba8();
        doll.image.width = img.width();
        doll.image.height = img.height();
        doll.image.pixels = img.into_vec();
    }

    for fragment in &mut manifest.fragments {
        if fragment.path.is_empty() {
            continue;
        }
        let bytes = files
            .get(Path::new(&fragment.path))
            .ok_or_else(|| anyhow!("ppd archive is missing fragment image {}", fragment.path))?;
        let img = image::load_from_memory(bytes)?.into_rgba8();
        fragment.image.width = img.width();
        fragment.image.height = img.height();
        fragment.image.pixels = img.into_vec();
    }

    PaperdollFactory::from_manifest(manifest)
}

/// Saves a `ppd` file using the given manifest to the path.
/// In-memory as well: encodes PNGs to buffers and streams them into the tar.
pub fn save<P>(manifest: &mut Manifest, path: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let output = std::fs::File::create(path)?;
    let mut archive = Builder::new(output);

    let mut append = |name: &str, data: &[u8]| -> Result<()> {
        let mut header = Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive.append_data(&mut header, name, data)?;
        Ok(())
    };

    let encode_png = |width: u32, height: u32, pixels: &[u8]| -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        image::write_buffer_with_format(
            &mut std::io::Cursor::new(&mut buf),
            pixels,
            width,
            height,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )?;
        Ok(buf)
    };

    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();

    for doll in &mut manifest.dolls {
        if doll.image.is_empty() {
            continue;
        }
        let filename = format!("doll_{}.png", doll.id());
        let png = encode_png(doll.image.width, doll.image.height, &doll.image.pixels)?;
        doll.path = filename.clone();
        entries.push((filename, png));
    }

    for fragment in &mut manifest.fragments {
        if fragment.image.is_empty() {
            continue;
        }
        let filename = format!("fragment_{}.png", fragment.id());
        let png = encode_png(
            fragment.image.width,
            fragment.image.height,
            &fragment.image.pixels,
        )?;
        fragment.path = filename.clone();
        entries.push((filename, png));
    }

    let manifest_str = serde_yaml::to_string(manifest)?;
    append(FILE_NAME_MANIFEST, manifest_str.as_bytes())?;
    for (name, data) in &entries {
        append(name, data)?;
    }

    archive.finish()?;
    Ok(())
}
