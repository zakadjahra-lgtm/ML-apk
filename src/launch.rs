//! Builds the Java command line for an installed version and starts it.
//!
//! NOTE (Android): Android has no system JVM and no desktop OpenGL/LWJGL.
//! To really run the game there you must ship an ARM64 JRE and an
//! LWJGL/GL translation layer (the approach PojavLauncher uses) and point
//! `Config::java_path` to the bundled `java` executable. This module only
//! produces and runs the standard Minecraft command line.

use crate::account::offline_uuid;
use crate::download::{active_libraries, client_jar_path};
use crate::version::*;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

pub struct LaunchConfig {
    pub java_path: String,
    pub root: PathBuf,
    pub game_dir: PathBuf,
    pub username: String,
    pub ram_mb: u32,
}

fn expand(list: &[Value]) -> Vec<String> {
    let mut out = Vec::new();
    for item in list {
        match item {
            Value::String(s) => out.push(s.clone()),
            Value::Object(o) => {
                let rules: Option<Vec<Rule>> = o
                    .get("rules")
                    .and_then(|r| serde_json::from_value(r.clone()).ok());
                if !rules_allow(&rules) {
                    continue;
                }
                match o.get("value") {
                    Some(Value::String(s)) => out.push(s.clone()),
                    Some(Value::Array(a)) => {
                        out.extend(a.iter().filter_map(|v| v.as_str().map(String::from)))
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    out
}

fn substitute(arg: &str, vars: &HashMap<&str, String>) -> String {
    let mut s = arg.to_string();
    for (k, v) in vars {
        s = s.replace(&format!("${{{k}}}"), v);
    }
    s
}

/// Returns the full command: executable first, then its arguments.
pub fn build_command(meta: &VersionMeta, cfg: &LaunchConfig) -> Result<Vec<String>> {
    let root = &cfg.root;
    let sep = if cfg!(target_os = "windows") { ";" } else { ":" };

    let mut cp: Vec<String> = active_libraries(meta, root)
        .into_iter()
        .map(|(_, p)| p.to_string_lossy().into_owned())
        .collect();
    cp.push(client_jar_path(root, &meta.id).to_string_lossy().into_owned());

    let natives = root.join("versions").join(&meta.id).join("natives");
    std::fs::create_dir_all(&natives)?;
    std::fs::create_dir_all(&cfg.game_dir)?;

    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("auth_player_name", cfg.username.clone());
    vars.insert("auth_uuid", offline_uuid(&cfg.username));
    vars.insert("auth_access_token", "0".into());
    vars.insert("auth_session", "0".into());
    vars.insert("clientid", String::new());
    vars.insert("auth_xuid", String::new());
    vars.insert("user_type", "legacy".into());
    vars.insert("user_properties", "{}".into());
    vars.insert("version_name", meta.id.clone());
    vars.insert("version_type", meta.kind.clone().unwrap_or_else(|| "release".into()));
    vars.insert("game_directory", cfg.game_dir.to_string_lossy().into_owned());
    vars.insert("assets_root", root.join("assets").to_string_lossy().into_owned());
    vars.insert("game_assets", root.join("assets").to_string_lossy().into_owned());
    vars.insert("assets_index_name", meta.asset_index.id.clone());
    vars.insert("natives_directory", natives.to_string_lossy().into_owned());
    vars.insert("library_directory", root.join("libraries").to_string_lossy().into_owned());
    vars.insert("classpath_separator", sep.into());
    vars.insert("classpath", cp.join(sep));
    vars.insert("launcher_name", "mc_launcher".into());
    vars.insert("launcher_version", env!("CARGO_PKG_VERSION").into());

    let (jvm_raw, game_raw): (Vec<String>, Vec<String>) = if let Some(a) = &meta.arguments {
        (expand(&a.jvm), expand(&a.game))
    } else if let Some(legacy) = &meta.legacy_arguments {
        (
            vec![
                "-Djava.library.path=${natives_directory}".into(),
                "-cp".into(),
                "${classpath}".into(),
            ],
            legacy.split_whitespace().map(String::from).collect(),
        )
    } else {
        bail!("version JSON has neither `arguments` nor `minecraftArguments`");
    };

    let mut cmd = vec![cfg.java_path.clone(), format!("-Xmx{}M", cfg.ram_mb)];
    cmd.extend(jvm_raw.iter().map(|a| substitute(a, &vars)));
    cmd.push(meta.main_class.clone());
    cmd.extend(game_raw.iter().map(|a| substitute(a, &vars)));
    Ok(cmd)
}

pub fn spawn(cmd: &[String], game_dir: &Path) -> Result<Child> {
    let (exe, args) = cmd.split_first().context("empty command")?;
    Command::new(exe)
        .args(args)
        .current_dir(game_dir)
        .spawn()
        .with_context(|| format!("could not start Java at `{exe}` - is a Java runtime installed/bundled?"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn expand_handles_strings_and_conditional_objects() {
        let list = vec![
            json!("--username"),
            json!({"rules":[{"action":"allow","features":{"is_demo_user":true}}],"value":"--demo"}),
            json!({"rules":[{"action":"allow"}],"value":["-A","-B"]}),
        ];
        assert_eq!(expand(&list), vec!["--username", "-A", "-B"]);
    }

    #[test]
    fn substitute_replaces_placeholders() {
        let mut v = HashMap::new();
        v.insert("auth_player_name", "Steve".to_string());
        assert_eq!(substitute("--username=${auth_player_name}", &v), "--username=Steve");
    }
}
