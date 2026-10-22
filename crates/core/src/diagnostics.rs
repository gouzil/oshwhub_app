//! Persistent diagnostics shared by native and desktop adapters.
use std::{
    fs::{self, OpenOptions},
    io,
    path::Path,
    sync::Mutex,
};

const MAX_LOG_BYTES: u64 = 8 * 1024 * 1024;
static INITIALIZED: Mutex<bool> = Mutex::new(false);

/// Initialize once per process; each new launch keeps one previous oversized log.
pub fn initialize(directory: &Path) -> io::Result<()> {
    let mut initialized = INITIALIZED
        .lock()
        .map_err(|_| io::Error::other("Diagnostic initialization lock was poisoned"))?;
    if *initialized {
        return Ok(());
    }
    fs::create_dir_all(directory)?;
    let path = directory.join("oshwhub-core.log");
    if fs::metadata(&path).is_ok_and(|metadata| metadata.len() > MAX_LOG_BYTES) {
        let previous = directory.join("oshwhub-core.previous.log");
        match fs::remove_file(&previous) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        fs::rename(&path, previous)?;
    }
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    tracing_subscriber::fmt()
        .json()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(Mutex::new(file))
        .try_init()
        .map_err(|_| io::Error::other("A global tracing subscriber is already installed"))?;

    // The default hook prints the payload, which may contain private input.
    std::panic::set_hook(Box::new(|info| {
        let panic_kind = if info.payload().is::<&str>() || info.payload().is::<String>() {
            "string"
        } else {
            "non_string"
        };
        let location = info.location();
        tracing::error!(
            panic_kind,
            source_file = location.map_or("unknown", |location| location.file()),
            source_line = location.map_or(0, |location| location.line()),
            source_column = location.map_or(0, |location| location.column()),
            "rust.panic"
        );
    }));
    *initialized = true;
    tracing::info!("diagnostics.ready");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, process::Command};

    #[test]
    fn file_logging_rotates_and_keeps_redacted_panics_after_process_exit() {
        let directory =
            std::env::temp_dir().join(format!("oshwhub-diagnostics-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let log = directory.join("oshwhub-core.log");
        fs::File::create(&log)
            .unwrap()
            .set_len(8 * 1024 * 1024 + 1)
            .unwrap();
        let previous = directory.join("oshwhub-core.previous.log");
        fs::write(&previous, "older log").unwrap();

        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "diagnostics::tests::logging_child",
                "--ignored",
                "--nocapture",
            ])
            .env("OSHWHUB_DIAGNOSTICS_TEST_DIR", &directory)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::metadata(previous).unwrap().len(), 8 * 1024 * 1024 + 1);
        let contents = fs::read_to_string(log).unwrap();
        let events = contents
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        let panic = events
            .iter()
            .find(|event| event["fields"]["message"] == "rust.panic")
            .unwrap();
        assert_eq!(panic["fields"]["panic_kind"], "string");
        assert!(panic["fields"]["source_line"].as_u64().unwrap() > 0);
        assert!(
            panic["fields"]["source_file"]
                .as_str()
                .unwrap()
                .ends_with("diagnostics.rs")
        );
        assert!(
            events
                .iter()
                .any(|event| event["fields"]["message"] == "diagnostics.child.complete")
        );
        assert!(!contents.contains("private-panic-payload"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("private-panic-payload"));
        fs::remove_dir_all(directory).unwrap();
    }

    // A separate process isolates the global subscriber/panic hook from other tests.
    #[test]
    #[ignore = "launched by the file logging test in an isolated process"]
    fn logging_child() {
        let Some(directory) = std::env::var_os("OSHWHUB_DIAGNOSTICS_TEST_DIR") else {
            return;
        };
        let directory = std::path::PathBuf::from(directory);
        super::initialize(&directory).unwrap();
        super::initialize(&directory).unwrap();
        let _ = std::panic::catch_unwind(|| panic!("private-panic-payload"));
        tracing::info!("diagnostics.child.complete");
    }
}
