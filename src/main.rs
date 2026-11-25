//! Desktop entry point, handy for testing the launcher without a phone.

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let base = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    mc_launcher::run(base.join(".mc_launcher"))
}
