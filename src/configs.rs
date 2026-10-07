use anyhow::{Context, Result};
use std::path::PathBuf;

const CONFIGS: &[(&str, &str)] = &[("configurations/psmux.conf", ".psmux.conf")];

pub fn distribute() -> Result<()> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .context("neither USERPROFILE nor HOME is set")?;
    let home = PathBuf::from(home);
    for (src, dest) in CONFIGS {
        let dest = home.join(dest);
        std::fs::copy(src, &dest)
            .with_context(|| format!("copying {src} to {}", dest.display()))?;
        println!("wrote {}", dest.display());
    }
    Ok(())
}
