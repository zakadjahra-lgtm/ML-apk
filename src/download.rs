//! Downloads a Minecraft version (client jar, libraries, assets) from Mojang's
//! public endpoints into the standard `.minecraft`-style layout, verifying SHA-1.

use crate::version::*;
use anyhow::{bail, Context, Result};
use futures_util::stream::{self, StreamExt};
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Progress callback: fraction 0.0..=1.0 and a status line.
pub type Progress = Arc<dyn Fn(f32, String) + Send + Sync>;

const PARALLEL_DOWNLOADS: usize = 16;

#[derive(Clone)]
struct Task {
    url: String,
    path: PathBuf,
    sha1: Option<String>,
}

fn sha1_of(bytes: &[u8]) -> String {
    hex::encode(Sha1::digest(bytes))
}

fn already_ok(path: &Path, sha1: Option<&str>) -> bool {
    if !path.exists() {
        return false;
    }
    match sha1 {
        None => true,
        Some(want) => std::fs::read(path).map(|b| sha1_of(&b) == want).unwrap_or(false),
    }
}

async fn download(client: &reqwest::Client, t: &Task) -> Result<()> {
    if already_ok(&t.path, t.sha1.as_deref()) {
        return Ok(());
    }
    if let Some(dir) = t.path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let mut last_err = None;
    for _ in 0..3 {
        let attempt: Result<()> = async {
            let bytes = client.get(&t.url).send().await?.error_for_status()?.bytes().await?;
            if let Some(want) = &t.sha1 {
                let got = sha1_of(&bytes);
                if &got != want {
                    bail!("SHA-1 mismatch for {} (expected {want}, got {got})", t.url);
                }
            }
            let tmp = t.path.with_extension("part");
            tokio::fs::write(&tmp, &bytes).await?;
            tokio::fs::rename(&tmp, &t.path).await?;
            Ok(())
        }
        .await;
        match attempt {
            Ok(()) => return Ok(()),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap()).with_context(|| format!("failed to download {}", t.url))
}

pub fn client_jar_path(root: &Path, id: &str) -> PathBuf {
    root.join("versions").join(id).join(format!("{id}.jar"))
}

pub fn library_path(root: &Path, a: &Artifact) -> Option<PathBuf> {
    a.path.as_ref().map(|p| root.join("libraries").join(p))
}

/// Libraries that apply to this OS/arch, as (artifact, local path) pairs.
pub fn active_libraries<'a>(meta: &'a VersionMeta, root: &Path) -> Vec<(&'a Artifact, PathBuf)> {
    meta.libraries
        .iter()
        .filter(|l| rules_allow(&l.rules))
        .filter_map(|l| l.downloads.as_ref()?.artifact.as_ref())
        .filter_map(|a| library_path(root, a).map(|p| (a, p)))
        .collect()
}

/// Installs everything needed to launch `entry` and returns its parsed metadata.
pub async fn install_version(
    client: &reqwest::Client,
    entry: &VersionEntry,
    root: &Path,
    progress: Progress,
) -> Result<VersionMeta> {
    progress(0.0, format!("Fetching {} info...", entry.id));

    let vdir = root.join("versions").join(&entry.id);
    tokio::fs::create_dir_all(&vdir).await?;
    let text = client.get(&entry.url).send().await?.error_for_status()?.text().await?;
    tokio::fs::write(vdir.join(format!("{}.json", entry.id)), &text).await?;
    let meta: VersionMeta = serde_json::from_str(&text).context("invalid version JSON")?;

    // Asset index first, since it tells us which asset objects exist.
    let index_path = root.join("assets").join("indexes").join(format!("{}.json", meta.asset_index.id));
    download(
        client,
        &Task {
            url: meta.asset_index.url.clone(),
            path: index_path.clone(),
            sha1: Some(meta.asset_index.sha1.clone()),
        },
    )
    .await?;
    let index: AssetIndex = serde_json::from_slice(&tokio::fs::read(&index_path).await?)
        .context("invalid asset index")?;

    let mut tasks: Vec<Task> = Vec::new();
    tasks.push(Task {
        url: meta.downloads.client.url.clone(),
        path: client_jar_path(root, &entry.id),
        sha1: meta.downloads.client.sha1.clone(),
    });
    for (a, path) in active_libraries(&meta, root) {
        tasks.push(Task { url: a.url.clone(), path, sha1: a.sha1.clone() });
    }
    let mut seen = std::collections::HashSet::new();
    for obj in index.objects.values() {
        if !seen.insert(obj.hash.clone()) {
            continue;
        }
        let prefix = &obj.hash[..2];
        tasks.push(Task {
            url: format!("{ASSET_BASE_URL}/{prefix}/{}", obj.hash),
            path: root.join("assets").join("objects").join(prefix).join(&obj.hash),
            sha1: Some(obj.hash.clone()),
        });
    }

    let total = tasks.len();
    let done = Arc::new(AtomicUsize::new(0));
    let results: Vec<Result<()>> = stream::iter(tasks)
        .map(|t| {
            let client = client.clone();
            let done = done.clone();
            let progress = progress.clone();
            async move {
                let r = download(&client, &t).await;
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                if n % 20 == 0 || n == total {
                    progress(n as f32 / total as f32, format!("Downloading files {n}/{total}"));
                }
                r
            }
        })
        .buffer_unordered(PARALLEL_DOWNLOADS)
        .collect()
        .await;

    for r in results {
        r?;
    }
    progress(1.0, "Download complete".into());
    Ok(meta)
}
