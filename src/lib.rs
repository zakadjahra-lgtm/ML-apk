//! Minecraft (Java Edition) offline launcher: Rust core + Slint UI.
//! Runs on Android (via `android_main`) and on desktop (via `main.rs`).

pub mod account;
pub mod config;
pub mod download;
pub mod launch;
pub mod version;

use anyhow::{bail, Context, Result};
use download::Progress;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

slint::include_modules!();

fn set_status(ui: &Mutex<Weak<AppWindow>>, text: String) {
    if let Ok(w) = ui.lock() {
        let _ = w.upgrade_in_event_loop(move |ui| ui.set_status(text.into()));
    }
}

fn set_busy(ui: &Mutex<Weak<AppWindow>>, busy: bool) {
    if let Ok(w) = ui.lock() {
        let _ = w.upgrade_in_event_loop(move |ui| ui.set_busy(busy));
    }
}

async fn play_flow(
    root: PathBuf,
    version_id: String,
    username: String,
    ui: Arc<Mutex<Weak<AppWindow>>>,
) -> Result<String> {
    if !account::valid_username(&username) {
        bail!("Invalid name: use 3-16 letters, digits or underscore");
    }
    if version_id.is_empty() {
        bail!("Pick a version first");
    }

    let mut cfg = config::Config::load(&root);
    cfg.username = username.clone();
    cfg.last_version = version_id.clone();
    cfg.save(&root)?;

    let client = reqwest::Client::builder()
        .user_agent(concat!("mc_launcher/", env!("CARGO_PKG_VERSION")))
        .build()?;

    let manifest = version::fetch_manifest(&client).await?;
    let entry = manifest
        .versions
        .iter()
        .find(|v| v.id == version_id)
        .with_context(|| format!("version {version_id} not found"))?
        .clone();

    let ui_progress = ui.clone();
    let progress: Progress = Arc::new(move |p, text| {
        if let Ok(w) = ui_progress.lock() {
            let _ = w.upgrade_in_event_loop(move |ui| {
                ui.set_progress(p);
                ui.set_status(text.into());
            });
        }
    });

    let meta = download::install_version(&client, &entry, &root, progress).await?;

    let lc = launch::LaunchConfig {
        java_path: cfg.java_path.clone(),
        root: root.clone(),
        game_dir: root.join("instances").join("default"),
        username,
        ram_mb: cfg.ram_mb,
    };
    let cmd = launch::build_command(&meta, &lc)?;
    let child = launch::spawn(&cmd, &lc.game_dir)?;
    Ok(format!("Game started (pid {})", child.id()))
}

/// Starts the launcher UI. `data_dir` is where versions, libraries and settings live.
pub fn run(data_dir: PathBuf) -> Result<()> {
    std::fs::create_dir_all(&data_dir)?;
    let ui = AppWindow::new()?;
    let cfg = config::Config::load(&data_dir);
    ui.set_username(cfg.username.clone().into());
    ui.set_selected_version(cfg.last_version.clone().into());

    let rt = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()?,
    );
    let weak = Arc::new(Mutex::new(ui.as_weak()));

    // Load the list of releases in the background.
    {
        let weak = weak.clone();
        let last = cfg.last_version.clone();
        rt.spawn(async move {
            set_status(&weak, "Loading version list...".into());
            let result = async {
                let client = reqwest::Client::new();
                version::fetch_manifest(&client).await
            }
            .await;
            match result {
                Ok(m) => {
                    let ids: Vec<String> = m
                        .versions
                        .iter()
                        .filter(|v| v.kind == "release")
                        .map(|v| v.id.clone())
                        .collect();
                    if let Ok(w) = weak.lock() {
                        let _ = w.upgrade_in_event_loop(move |ui| {
                            if ui.get_selected_version().is_empty() || last.is_empty() {
                                if let Some(first) = ids.first() {
                                    ui.set_selected_version(first.as_str().into());
                                }
                            }
                            let model: Vec<SharedString> =
                                ids.iter().map(|s| s.as_str().into()).collect();
                            ui.set_versions(ModelRc::new(VecModel::from(model)));
                            ui.set_status("Ready".into());
                        });
                    }
                }
                Err(e) => set_status(&weak, format!("Could not load versions: {e}")),
            }
        });
    }

    // Play button.
    {
        let weak = weak.clone();
        let rt2 = rt.clone();
        let root = data_dir.clone();
        ui.on_play(move |version, name| {
            let weak = weak.clone();
            let root = root.clone();
            let version = version.to_string();
            let name = name.trim().to_string();
            set_busy(&weak, true);
            rt2.spawn(async move {
                match play_flow(root, version, name, weak.clone()).await {
                    Ok(msg) => set_status(&weak, msg),
                    Err(e) => set_status(&weak, format!("Error: {e:#}")),
                }
                set_busy(&weak, false);
            });
        });
    }

    ui.run()?;
    Ok(())
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: slint::android::AndroidApp) {
    let data_dir = app
        .internal_data_path()
        .unwrap_or_else(|| PathBuf::from("/data/local/tmp"))
        .join("minecraft");
    slint::android::init(app).expect("failed to init Slint on Android");
    if let Err(e) = run(data_dir) {
        eprintln!("launcher error: {e:#}");
    }
}
