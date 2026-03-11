use camino::Utf8PathBuf;

fn cargo_target_tmp_root() -> Utf8PathBuf {
    if let Ok(dir) = std::env::var("CARGO_TARGET_TMPDIR") {
        return Utf8PathBuf::from(dir);
    }

    // Unit tests in this crate run from target/<profile>/deps/*. Derive target/tmp from that.
    let exe = std::env::current_exe().expect("failed to determine current test binary path");
    let exe = Utf8PathBuf::from_path_buf(exe).expect("test binary path is not valid UTF-8");
    let target_dir = exe
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .expect("failed to derive Cargo target dir from test binary path");
    target_dir.join("tmp")
}

/// Returns a per-test directory under Cargo's test temp root.
pub(crate) fn unit_test_tmp_dir(suite: &str, test: &str) -> Utf8PathBuf {
    let dir = cargo_target_tmp_root().join("unit").join(suite).join(test);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
