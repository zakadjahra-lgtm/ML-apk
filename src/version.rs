//! Types for Mojang's version manifest and per-version JSON, plus rule evaluation.

use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
pub const ASSET_BASE_URL: &str = "https://resources.download.minecraft.net";

// ---------- version manifest ----------

#[derive(Debug, Clone, Deserialize)]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionManifest {
    pub versions: Vec<VersionEntry>,
}

pub async fn fetch_manifest(client: &reqwest::Client) -> Result<VersionManifest> {
    Ok(client
        .get(MANIFEST_URL)
        .send()
        .await?
        .error_for_status()?
        .json::<VersionManifest>()
        .await?)
}

// ---------- per-version JSON ----------

#[derive(Debug, Clone, Deserialize)]
pub struct Artifact {
    pub path: Option<String>,
    pub sha1: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibDownloads {
    pub artifact: Option<Artifact>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibDownloads>,
    pub rules: Option<Vec<Rule>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub action: String,
    pub os: Option<OsRule>,
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Downloads {
    pub client: Artifact,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Value>,
    #[serde(default)]
    pub jvm: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JavaVersion {
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionMeta {
    pub id: String,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    #[serde(rename = "assetIndex")]
    pub asset_index: AssetIndexRef,
    pub downloads: Downloads,
    pub libraries: Vec<Library>,
    pub arguments: Option<Arguments>,
    #[serde(rename = "minecraftArguments")]
    pub legacy_arguments: Option<String>,
    #[serde(rename = "javaVersion")]
    pub java_version: Option<JavaVersion>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AssetObject {
    pub hash: String,
}

#[derive(Debug, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
}

// ---------- rules ----------

fn current_os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        // Android is Linux-based, so it uses the Linux rules.
        "linux"
    }
}

fn current_arch_names() -> &'static [&'static str] {
    if cfg!(target_arch = "aarch64") {
        &["arm64", "aarch64"]
    } else if cfg!(target_arch = "x86_64") {
        &["x86_64", "amd64"]
    } else if cfg!(target_arch = "x86") {
        &["x86"]
    } else {
        &[]
    }
}

fn rule_matches(rule: &Rule) -> bool {
    // No optional launcher features (demo mode, custom resolution...) are enabled.
    if let Some(f) = &rule.features {
        if !f.is_empty() {
            return false;
        }
    }
    if let Some(os) = &rule.os {
        if let Some(name) = &os.name {
            if name != current_os_name() {
                return false;
            }
        }
        if let Some(arch) = &os.arch {
            if !current_arch_names().contains(&arch.as_str()) {
                return false;
            }
        }
    }
    true
}

/// Mojang rule semantics: with rules present the item is disallowed by default,
/// and every matching rule overrides the result in order.
pub fn rules_allow(rules: &Option<Vec<Rule>>) -> bool {
    match rules {
        None => true,
        Some(rs) if rs.is_empty() => true,
        Some(rs) => {
            let mut allowed = false;
            for r in rs {
                if rule_matches(r) {
                    allowed = r.action == "allow";
                }
            }
            allowed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_rules_means_allowed() {
        assert!(rules_allow(&None));
    }

    #[test]
    fn disallow_for_current_os() {
        let rules = Some(vec![
            Rule { action: "allow".into(), os: None, features: None },
            Rule {
                action: "disallow".into(),
                os: Some(OsRule { name: Some(current_os_name().into()), arch: None }),
                features: None,
            },
        ]);
        assert!(!rules_allow(&rules));
    }

    #[test]
    fn feature_rules_never_match() {
        let mut f = HashMap::new();
        f.insert("is_demo_user".to_string(), true);
        let rules = Some(vec![Rule { action: "allow".into(), os: None, features: Some(f) }]);
        assert!(!rules_allow(&rules));
    }
}
