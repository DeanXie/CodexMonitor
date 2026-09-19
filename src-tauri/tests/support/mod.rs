use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct IsolatedProcessEnvironment {
    root: PathBuf,
    appdata: PathBuf,
    local_appdata: PathBuf,
    userprofile: PathBuf,
    codex_home: PathBuf,
    temp: PathBuf,
    xdg_data: PathBuf,
    xdg_cache: PathBuf,
}

impl IsolatedProcessEnvironment {
    pub fn new(test_root: &Path) -> Result<Self, String> {
        if !test_root.is_absolute() {
            return Err("isolated process root must be absolute".to_string());
        }
        let root = test_root.join("process-environment");
        let environment = Self {
            appdata: root.join("appdata/roaming"),
            local_appdata: root.join("appdata/local"),
            userprofile: root.join("userprofile"),
            codex_home: root.join("codex-home"),
            temp: root.join("temp"),
            xdg_data: root.join("xdg/data"),
            xdg_cache: root.join("xdg/cache"),
            root,
        };
        for path in environment.paths() {
            fs::create_dir_all(path)
                .map_err(|error| format!("failed to create isolated process root: {error}"))?;
        }
        environment.assert_confined()?;
        Ok(environment)
    }

    pub fn command(&self, program: impl AsRef<std::ffi::OsStr>) -> Command {
        let mut command = Command::new(program);
        command
            .env("APPDATA", &self.appdata)
            .env("LOCALAPPDATA", &self.local_appdata)
            .env("USERPROFILE", &self.userprofile)
            .env("HOME", &self.userprofile)
            .env("CODEX_HOME", &self.codex_home)
            .env("TEMP", &self.temp)
            .env("TMP", &self.temp)
            .env("XDG_DATA_HOME", &self.xdg_data)
            .env("XDG_CACHE_HOME", &self.xdg_cache)
            .env_remove("CODEX_MONITOR_DAEMON_TOKEN");
        command
    }

    pub fn assert_confined(&self) -> Result<(), String> {
        let canonical_root = fs::canonicalize(&self.root)
            .map_err(|error| format!("failed to resolve isolated process root: {error}"))?;
        for path in self.paths() {
            let canonical = fs::canonicalize(path)
                .map_err(|error| format!("failed to resolve isolated child root: {error}"))?;
            if !canonical.starts_with(&canonical_root) {
                return Err(format!(
                    "isolated child root escaped test root: {}",
                    canonical.display()
                ));
            }
        }
        Ok(())
    }

    fn paths(&self) -> [&Path; 7] {
        [
            &self.appdata,
            &self.local_appdata,
            &self.userprofile,
            &self.codex_home,
            &self.temp,
            &self.xdg_data,
            &self.xdg_cache,
        ]
    }
}
