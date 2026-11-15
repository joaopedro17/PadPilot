use std::fs;

/// One test, because logging is set up once per process.
#[test]
fn writes_timestamped_lines_and_records_panics() {
    let dir = std::env::temp_dir().join(format!("padpilot-log-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);

    let guard = padpilot_logging::init(&dir, "padpilot", tracing::Level::INFO).unwrap();
    tracing::info!(target: "controller", "connected");
    tracing::debug!(target: "stick", "too detailed for the default level");
    let _ = std::panic::catch_unwind(|| panic!("boom"));
    drop(guard);

    let file = fs::read_dir(&dir).unwrap().next().unwrap().unwrap();
    let name = file.file_name().into_string().unwrap();
    assert!(
        name.starts_with("padpilot.") && name.ends_with(".log"),
        "{name}"
    );

    let log = fs::read_to_string(file.path()).unwrap();
    let line = log.lines().next().unwrap();
    // 2026-11-15 12:00:00.123  INFO controller: connected
    let (stamp, rest) = line.split_at(23);
    assert_eq!(
        stamp.len() - stamp.rfind('.').unwrap(),
        4,
        "milliseconds in {line}"
    );
    assert!(
        stamp.starts_with("20") && stamp.as_bytes()[10] == b' ',
        "{line}"
    );
    assert_eq!(rest.trim_start(), "INFO controller: connected");
    assert!(!log.contains("too detailed"), "{log}");
    assert!(
        log.contains("ERROR panic:") && log.contains("boom"),
        "{log}"
    );

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn text_files_get_windows_line_endings_a_folder_and_no_leftovers() {
    let dir = std::env::temp_dir().join(format!("padpilot-write-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("deeper").join("settings.toml");
    padpilot_logging::write_text_file(&path, "a = 1\nb = 2\r\n").unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "a = 1\r\nb = 2\r\n"
    );
    padpilot_logging::write_text_file(&path, "c = 3\n").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "c = 3\r\n");
    let files: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().collect();
    assert_eq!(files.len(), 1, "only the file itself is left");
    std::fs::remove_dir_all(&dir).unwrap();
}
