/// Run `bpt` at a specified root with given arguments along with implied common test arguments
///
/// Same as `run!` but takes an explicit root path instead of using `per_test_path!()`.
#[macro_export]
macro_rules! run_at {
    ($root:expr, $($arg:expr),*) => {{
        let root = $root;
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_bpt"))
            .args(&["-SVy",
                  "-R", root,
                  "-O", root,
                  $($arg), *])
            .output()
            .expect("failed to execute bpt");
        if result.status.success() {
            Ok(String::from_utf8_lossy(&result.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&result.stderr).to_string())
        }
    }}
}

/// Run `bpt` with given arguments along with implied common test arguments
///
/// Common arguments:
/// - Do not sign anything (`-S`).  Decrypting the private key takes a while and slows down tests.
/// - Do not verify signatures (`-V`).  We're not signing anything.
/// - Auto-accept prompts to run non-interactively (`-y`).
/// - Run against per-test root (`-R`).
/// - Output to per-test root (`-O`).
#[macro_export]
macro_rules! run {
    ($($arg:expr),*) => {{
        run_at!(per_test_path!(), $($arg),*)
    }}
}

/// Run `bpt` at a specified root with custom environment variables.
pub fn run_bpt_at_with_envs(
    root: &str,
    args: &[&str],
    envs: &[(&str, &str)],
) -> Result<String, String> {
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_bpt"));
    cmd.args(["-SVy", "-R", root, "-O", root]);
    cmd.args(args);
    for (key, value) in envs {
        cmd.env(key, value);
    }
    let result = cmd.output().expect("failed to execute bpt");
    if result.status.success() {
        Ok(String::from_utf8_lossy(&result.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&result.stderr).to_string())
    }
}

/// Run `bpt sign` with proper key setup
///
/// Cannot use `run!()` because it skips key usage.
#[macro_export]
macro_rules! run_bpt_sign {
    ($($arg:expr),*) => {{
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_bpt"))
            .args(&["-y",
                  "-R", per_test_path!(),
                  "-O", per_test_path!(),
                  "-P", common_path!("etc/bpt/private-keys/test-key-password-is-bpt.key"),
                  "--priv-key-passphrase-file", common_path!("etc/bpt/private-keys/test-key-password-is-bpt.passphrase"),
                  "sign",
                  $($arg), *])
            .output()
            .expect("failed to execute bpt");

        if result.status.success() {
            Ok(String::from_utf8_lossy(&result.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&result.stderr).to_string())
        }
    }}
}

/// Run `bpt verify` with while actually verifying
///
/// Cannot use `run!()` because it skips key usage.
#[macro_export]
macro_rules! run_bpt_verify {
    ($($arg:expr),*) => {{
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_bpt"))
            .args(&["-Sy",
                  "-R", per_test_path!(),
                  "-O", per_test_path!(),
                  "verify",
                  $($arg), *])
            .output()
            .expect("failed to execute bpt");
        if result.status.success() {
            Ok(String::from_utf8_lossy(&result.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&result.stderr).to_string())
        }
    }}
}

/// Run `bpt make-repo` without specifying common `-O` flag.
#[macro_export]
macro_rules! run_bpt_make_repo {
    ($($arg:expr),*) => {{
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_bpt"))
            .args(&["-ySV",
                  "-R", per_test_path!(),
                  "make-repo",
                  $($arg), *])
            .output()
            .expect("failed to execute bpt");
        if result.status.success() {
            Ok(String::from_utf8_lossy(&result.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&result.stderr).to_string())
        }
    }}
}

/// Run `bpt make-repo` while specifying stdin for prompt
#[macro_export]
macro_rules! run_bpt_make_repo_prompt {
    ($prompt:expr,$($arg:expr),*) => {{
        use std::io::Write;

        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_bpt"))
            .args(&[
                  "-SV",
                  "-R", per_test_path!(),
                  "-P", common_path!("etc/bpt/private-keys/test-key-password-is-bpt.key"),
                  "make-repo",
                  $($arg), *])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("failed to execute bpt");

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all($prompt).expect("failed to write to stdin");
        }

        let result = child.wait_with_output().expect("failed to wait on bpt");

        if result.status.success() {
            Ok(String::from_utf8_lossy(&result.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&result.stderr).to_string())
        }
    }}
}
