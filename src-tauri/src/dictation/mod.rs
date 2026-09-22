// Dictation is intentionally disabled for this build. Keeping the stub preserves
// the IPC contract without pulling native Whisper/audio dependencies into Cargo.
use std::path::PathBuf;

#[path = "stub.rs"]
mod imp;

pub(crate) use imp::*;

fn resolve_dictation_model_dir(app_data_dir: Result<PathBuf, String>) -> Result<PathBuf, String> {
    app_data_dir
        .map(|root| root.join("models").join("whisper"))
        .map_err(|error| {
            format!("Failed to resolve application data directory for dictation: {error}")
        })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn app_data_resolution_failure_does_not_use_a_valid_model_under_cwd() {
        let _guard = cwd_lock().lock().expect("lock cwd test");
        let original_cwd = std::env::current_dir().expect("read cwd");
        let fixture_root = std::env::temp_dir().join(format!(
            "codex-monitor-dictation-path-{}",
            std::process::id()
        ));
        let apparent_model = fixture_root.join("models/whisper/ggml-base.bin");
        fs::create_dir_all(apparent_model.parent().expect("model parent"))
            .expect("create model directory");
        fs::write(&apparent_model, b"not-a-real-model").expect("write model fixture");
        std::env::set_current_dir(&fixture_root).expect("set fixture cwd");

        let result = super::resolve_dictation_model_dir(Err("resolver unavailable".to_string()));

        std::env::set_current_dir(&original_cwd).expect("restore cwd");
        fs::remove_dir_all(&fixture_root).expect("remove fixture");
        let error = result.expect_err("app_data_dir failure must disable dictation storage");
        assert!(error.contains("application data directory"));
        assert!(!error.contains(fixture_root.to_string_lossy().as_ref()));
    }

    #[test]
    fn resolved_app_data_directory_owns_the_dictation_model_root() {
        let root = PathBuf::from("C:/isolated/profile");
        assert_eq!(
            super::resolve_dictation_model_dir(Ok(root.clone())).expect("resolve model root"),
            root.join("models/whisper")
        );
    }
}
