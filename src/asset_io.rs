//! Robust HTTP asset reader for the web build.
//!
//! Bevy 0.19's built-in `HttpWasmAssetReader` does, on a `200` response,
//! `JsFuture::from(resp.array_buffer().unwrap()).await.unwrap()` — so if the
//! HTTP/2 stream is reset *after* the 200 headers arrive (the
//! `net::ERR_HTTP2_PROTOCOL_ERROR ... 200 (OK)` case the game hits when bevy
//! bulk-loads ~380 sprites over a single connection), reading the body rejects
//! and that `.unwrap()` **panics** — taking the whole game down. A single
//! transient fetch failure across hundreds of assets is therefore fatal, which
//! is especially likely over a long-haul / lossy connection.
//!
//! This reader mirrors bevy's fetch logic but treats every failure as a
//! recoverable `Result` and *retries* transient ones with a short backoff
//! instead of unwrapping. `403`/`404` map to `NotFound` (no retry); other `4xx`
//! are returned as hard errors.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::asset::io::{AssetReader, AssetReaderError, PathStream, Reader, VecReader};
use js_sys::{JSON, Promise, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Response, Window, WorkerGlobalScope};

/// Total fetch attempts per asset before giving up (1 initial try + retries).
const MAX_ATTEMPTS: u32 = 4;

/// Base backoff between attempts; grows linearly (120ms, 240ms, 360ms…). The
/// pause lets the browser establish a fresh stream/connection after a reset.
const BACKOFF_MS: i32 = 120;

pub struct RobustHttpAssetReader {
    /// Root the asset paths are joined onto, e.g. `assets` → `/assets/<path>`.
    root: PathBuf,
}

impl RobustHttpAssetReader {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

/// Outcome of a single fetch attempt.
enum Attempt {
    Ok(Vec<u8>),
    /// 403/404 — the asset isn't there; do not retry.
    NotFound,
    /// fetch/body rejection or a 5xx — worth retrying.
    Transient(std::io::Error),
    /// Other 4xx or an unsupported environment — a real error; do not retry.
    Fatal(std::io::Error),
}

fn io(msg: impl Into<String>) -> std::io::Error {
    std::io::Error::other(msg.into())
}

/// Turn a rejected `JsValue` into a descriptive `io::Error`.
fn jsv_io(ctx: &str, v: JsValue) -> std::io::Error {
    let detail = JSON::stringify(&v)
        .ok()
        .and_then(|s| s.as_string())
        .unwrap_or_else(|| "<unstringifiable JsValue>".to_owned());
    io(format!("{ctx}: {detail}"))
}

/// Start a `fetch()` against the page's global scope (window or worker).
fn fetch_promise(url: &str) -> Result<Promise, std::io::Error> {
    let global = js_sys::global();
    if let Ok(window) = global.clone().dyn_into::<Window>() {
        Ok(window.fetch_with_str(url))
    } else if let Ok(worker) = global.dyn_into::<WorkerGlobalScope>() {
        Ok(worker.fetch_with_str(url))
    } else {
        Err(io(
            "no fetch-capable JS global (neither Window nor WorkerGlobalScope)",
        ))
    }
}

/// Resolve after roughly `ms` milliseconds via the host's `setTimeout`. Falls
/// back to resolving immediately if no timer is available.
async fn sleep(ms: i32) {
    let promise = Promise::new(&mut |resolve, _reject| {
        let global = js_sys::global();
        let scheduled = if let Ok(window) = global.clone().dyn_into::<Window>() {
            window
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms)
                .is_ok()
        } else if let Ok(worker) = global.dyn_into::<WorkerGlobalScope>() {
            worker
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms)
                .is_ok()
        } else {
            false
        };
        if !scheduled {
            let _ = resolve.call0(&JsValue::NULL);
        }
    });
    let _ = JsFuture::from(promise).await;
}

impl RobustHttpAssetReader {
    async fn attempt(&self, url: &str) -> Attempt {
        let promise = match fetch_promise(url) {
            Ok(p) => p,
            Err(e) => return Attempt::Fatal(e),
        };
        let resp_value = match JsFuture::from(promise).await {
            Ok(v) => v,
            // Network-level failure (DNS, connection refused/reset before headers).
            Err(e) => return Attempt::Transient(jsv_io("fetch", e)),
        };
        let resp: Response = match resp_value.dyn_into() {
            Ok(r) => r,
            Err(e) => return Attempt::Transient(jsv_io("cast Response", e)),
        };
        match resp.status() {
            200 => {
                let ab = match resp.array_buffer() {
                    Ok(p) => p,
                    Err(e) => return Attempt::Transient(jsv_io("array_buffer", e)),
                };
                match JsFuture::from(ab).await {
                    Ok(data) => Attempt::Ok(Uint8Array::new(&data).to_vec()),
                    // Stream reset mid-body after a 200 — the exact case bevy unwraps on.
                    Err(e) => Attempt::Transient(jsv_io("read body", e)),
                }
            }
            403 | 404 => Attempt::NotFound,
            s if s >= 500 => Attempt::Transient(io(format!("HTTP {s}"))),
            s => Attempt::Fatal(io(format!("HTTP {s}"))),
        }
    }

    async fn fetch_retrying(&self, full: &Path) -> Result<Vec<u8>, AssetReaderError> {
        let url = full.to_string_lossy();
        let mut last = io("fetch failed");
        for attempt in 0..MAX_ATTEMPTS {
            match self.attempt(&url).await {
                Attempt::Ok(bytes) => return Ok(bytes),
                Attempt::NotFound => return Err(AssetReaderError::NotFound(full.to_path_buf())),
                Attempt::Fatal(e) => return Err(AssetReaderError::Io(Arc::new(e))),
                Attempt::Transient(e) => {
                    last = e;
                    if attempt + 1 < MAX_ATTEMPTS {
                        sleep(BACKOFF_MS * (attempt as i32 + 1)).await;
                    }
                }
            }
        }
        Err(AssetReaderError::Io(Arc::new(last)))
    }
}

impl AssetReader for RobustHttpAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let full = self.root.join(path);
        self.fetch_retrying(&full).await.map(VecReader::new)
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        // The web build runs with `AssetMetaCheck::Never`, so meta is never read;
        // the explicit `VecReader` type just unifies the `impl Reader` return.
        Err::<VecReader, _>(AssetReaderError::NotFound(self.root.join(path)))
    }

    async fn read_directory<'a>(
        &'a self,
        _path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        Err(AssetReaderError::Io(Arc::new(io(
            "directory listing is not supported over HTTP",
        ))))
    }

    async fn is_directory<'a>(&'a self, _path: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(false)
    }
}
