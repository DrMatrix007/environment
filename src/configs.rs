use anyhow::{Context, Result};
use std::path::PathBuf;

const CONFIGS: &[(&str, &str)] = &[
    ("configurations/tuios/config.toml", "tuios/config.toml"),
    ("configurations/tuios/tools.tape", "tuios/tools.tape"),
];

pub fn distribute() -> Result<()> {
    let local_app_data = std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is not set")?;
    let local_app_data = PathBuf::from(local_app_data);
    for (src, dest) in CONFIGS {
        let dest = local_app_data.join(dest);
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        std::fs::copy(src, &dest).with_context(|| format!("copying {src} to {}", dest.display()))?;
        println!("wrote {}", dest.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct LocalAppDataGuard(Option<std::ffi::OsString>);

    impl Drop for LocalAppDataGuard {
        fn drop(&mut self) {
            unsafe {
                match self.0.take() {
                    Some(value) => std::env::set_var("LOCALAPPDATA", value),
                    None => std::env::remove_var("LOCALAPPDATA"),
                }
            }
        }
    }

    #[test]
    fn distribute_copies_config_into_localappdata() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _restore = LocalAppDataGuard(std::env::var_os("LOCALAPPDATA"));

        let tmp = std::env::temp_dir().join(format!("environment-test-{}", std::process::id()));
        unsafe {
            std::env::set_var("LOCALAPPDATA", &tmp);
        }

        let result = distribute();

        assert!(result.is_ok());
        assert!(tmp.join("tuios").is_dir());
        assert_eq!(
            std::fs::read(tmp.join("tuios/config.toml")).unwrap(),
            std::fs::read("configurations/tuios/config.toml").unwrap()
        );
        assert_eq!(
            std::fs::read(tmp.join("tuios/tools.tape")).unwrap(),
            std::fs::read("configurations/tuios/tools.tape").unwrap()
        );

        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn distribute_errors_without_localappdata() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _restore = LocalAppDataGuard(std::env::var_os("LOCALAPPDATA"));

        unsafe {
            std::env::remove_var("LOCALAPPDATA");
        }

        let result = distribute();

        assert!(result.is_err());
        assert!(format!("{:#}", result.unwrap_err()).contains("LOCALAPPDATA"));
    }
}
