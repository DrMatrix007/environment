use anyhow::{Context, Result};
use std::path::PathBuf;

const SRC: &str = "configurations/herdr/config.toml";

pub fn distribute() -> Result<()> {
    let app_data = std::env::var_os("APPDATA").context("APPDATA is not set")?;
    let dest = PathBuf::from(app_data).join("herdr/config.toml");
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::copy(SRC, &dest).with_context(|| format!("copying {SRC} to {}", dest.display()))?;
    println!("wrote {}", dest.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct AppDataGuard(Option<std::ffi::OsString>);

    impl Drop for AppDataGuard {
        fn drop(&mut self) {
            unsafe {
                match self.0.take() {
                    Some(value) => std::env::set_var("APPDATA", value),
                    None => std::env::remove_var("APPDATA"),
                }
            }
        }
    }

    #[test]
    fn distribute_copies_config_into_appdata() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _restore = AppDataGuard(std::env::var_os("APPDATA"));

        let tmp = std::env::temp_dir().join(format!("environment-test-{}", std::process::id()));
        unsafe {
            std::env::set_var("APPDATA", &tmp);
        }

        let result = distribute();

        assert!(result.is_ok());
        assert_eq!(
            std::fs::read(tmp.join("herdr/config.toml")).unwrap(),
            std::fs::read(SRC).unwrap()
        );

        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn distribute_errors_without_appdata() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _restore = AppDataGuard(std::env::var_os("APPDATA"));

        unsafe {
            std::env::remove_var("APPDATA");
        }

        let result = distribute();

        assert!(result.is_err());
        assert!(format!("{:#}", result.unwrap_err()).contains("APPDATA"));
    }
}
