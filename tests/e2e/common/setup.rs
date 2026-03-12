// Test setup.  This ensures both common resources and per-test environment are setup.

use crate::{
    common_path,
    e2e::common::file_server::{assert_file_server_ready, launch_file_server},
    repo_path, repo_url,
};

pub static COMMON_SETUP: std::sync::LazyLock<()> = std::sync::LazyLock::new(common_setup);

/// Per-test setup
#[macro_export]
macro_rules! setup_test {
    () => {{
        // Ensure common setup is complete
        // LazyLock ensures this only happens once across all tests.
        *$crate::e2e::common::setup::COMMON_SETUP;

        // Clear per-test state from possible previous run
        std::fs::remove_dir_all(per_test_path!()).unwrap_or(());

        // Copy common configs into per-test root so mutable files like the world file remain
        // isolated between tests.
        $crate::e2e::common::setup::copy_dir(common_path!("etc/bpt"), per_test_path!("etc/bpt"));

        // Copy synced repo indexes into per-test root so tests can look up packages by pkgid
        // without modifying the shared common state.
        $crate::e2e::common::setup::copy_dir(common_path!("var"), per_test_path!("var"));
    }};
}

/// Recursively copy a directory tree.
pub fn copy_dir(src: &str, dst: &str) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let src_path = entry.path();
        let dst_path = std::path::Path::new(dst).join(entry.file_name());
        if src_path.is_dir() {
            copy_dir(src_path.to_str().unwrap(), dst_path.to_str().unwrap());
        } else {
            std::fs::copy(&src_path, &dst_path).unwrap();
        }
    }
}

/// Setup environment common to multiple tests.  Only runs once across all tests.
pub fn common_setup() {
    use std::fs::*;
    use std::io::Write;
    use std::os::unix::fs::symlink;

    let cwd = std::env::current_dir().unwrap();
    let cwd = cwd.to_str().unwrap();

    // Clear state from possible previous run
    remove_dir_all(common_path!()).unwrap_or(());

    // Common bpt.conf
    create_dir_all(common_path!("etc/bpt")).unwrap();
    symlink(
        format!("{cwd}/assets/default-configs/bpt.conf"),
        common_path!("etc/bpt/bpt.conf"),
    )
    .unwrap();

    // Common key configuration
    create_dir_all(common_path!("etc/bpt/keys")).unwrap();
    symlink(
        format!("{cwd}/tests/keys/test-key-password-is-bpt.pub"),
        common_path!("etc/bpt/keys/test-key-password-is-bpt.pub"),
    )
    .unwrap();
    symlink(
        format!("{cwd}/tests/keys/test-key-no-password.pub"),
        common_path!("etc/bpt/keys/test-key-no-password.pub"),
    )
    .unwrap();
    create_dir_all(common_path!("etc/bpt/private-keys")).unwrap();
    symlink(
        format!("{cwd}/tests/keys/test-key-password-is-bpt.key"),
        common_path!("etc/bpt/private-keys/test-key-password-is-bpt.key"),
    )
    .unwrap();
    symlink(
        format!("{cwd}/tests/keys/test-key-password-is-bpt.passphrase"),
        common_path!("etc/bpt/private-keys/test-key-password-is-bpt.passphrase"),
    )
    .unwrap();

    // Common repo configuration
    create_dir_all(common_path!("etc/bpt/repos")).unwrap();
    let mut file = File::create(common_path!("etc/bpt/repos/localhost")).unwrap();
    writeln!(file, "{}", repo_path!("noarch.pkgidx")).unwrap();
    writeln!(file, "{}", repo_path!("noarch.fileidx")).unwrap();
    writeln!(file, "{}", repo_url!("bbuild.pkgidx")).unwrap();

    // Common repo bbuild files
    create_dir_all(repo_path!("")).unwrap();
    symlink(
        format!("{cwd}/tests/bbuilds/fakeblock@1.0.0.bbuild"),
        repo_path!("fakeblock@1.0.0.bbuild"),
    )
    .unwrap();
    symlink(
        format!("{cwd}/tests/bbuilds/fakeblock-songs@1.0.0.bbuild"),
        repo_path!("fakeblock-songs@1.0.0.bbuild"),
    )
    .unwrap();
    symlink(
        format!("{cwd}/tests/bbuilds/fakeblock-song-gen@1.0.0.bbuild"),
        repo_path!("fakeblock-song-gen@1.0.0.bbuild"),
    )
    .unwrap();
    symlink(
        format!("{cwd}/tests/bbuilds/aaa-consumer@1.0.0.bbuild"),
        repo_path!("aaa-consumer@1.0.0.bbuild"),
    )
    .unwrap();
    symlink(
        format!("{cwd}/tests/bbuilds/zzz-helper@1.0.0.bbuild"),
        repo_path!("zzz-helper@1.0.0.bbuild"),
    )
    .unwrap();

    // Common repo binary files
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_bpt"))
        .args([
            "-SVy",
            "-R",
            common_path!(),
            "-O",
            repo_path!(),
            "make-repo",
        ])
        .output()
        .expect("failed to execute bpt make-repo");
    assert!(
        output.status.success(),
        "bpt make-repo failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Launch file server for `http://` tests
    launch_file_server();
    assert_file_server_ready("fakeblock@1.0.0.bbuild");

    // Sync repo indexes so tests can look up packages by pkgid
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_bpt"))
        .args(["-SVy", "-R", common_path!(), "sync"])
        .output()
        .expect("failed to execute bpt sync");
    assert!(
        output.status.success(),
        "bpt sync failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
