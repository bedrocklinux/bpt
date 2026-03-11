use crate::smoke::common::*;
use crate::*;
use ::function_name::named;
use std::collections::BTreeSet;

fn run_cli_in_root(root: &str, args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_bpt"))
        .args(["-SVy", "-R", root, "-O", root])
        .args(args)
        .output()
        .expect("failed to execute bpt")
}

fn assert_help_output(output: &std::process::Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage:"),
        "{label} help missing Usage:\n{stdout}"
    );
    assert!(
        !stdout.trim().is_empty(),
        "{label} help should not be empty"
    );
}

fn assert_clean_failure(output: &std::process::Output, label: &str) {
    assert!(
        !output.status.success(),
        "{label} unexpectedly succeeded:\nstdout:{}\nstderr:{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.trim().is_empty(),
        "{label} stderr should not be empty"
    );
    assert!(
        stderr.contains("ERROR:") || stderr.contains("error:") || stderr.contains("Usage:"),
        "{label} stderr should contain a clean error message:\n{stderr}"
    );
    assert!(
        !stderr.contains("panicked at"),
        "{label} stderr should not contain a panic:\n{stderr}"
    );
    assert!(
        !stderr.contains("stack backtrace"),
        "{label} stderr should not contain a backtrace:\n{stderr}"
    );
}

#[test]
fn top_level_help_works() {
    assert_help_output(&run_cli(&["--help"]), "top-level --help");
    assert_help_output(&run_cli(&["-h"]), "top-level -h");
}

#[test]
fn all_subcommands_help_work() {
    for subcommand in EXPECTED_COMMANDS {
        let output = run_cli(&[subcommand, "--help"]);
        assert_help_output(&output, subcommand);

        let output = run_cli(&[subcommand, "-h"]);
        assert_help_output(&output, subcommand);
    }
}

#[test]
fn top_level_parser_failures_are_clean() {
    assert_clean_failure(&run_cli(&[]), "missing subcommand");
    assert_clean_failure(
        &run_cli(&["definitely-not-a-command"]),
        "unknown subcommand",
    );
    assert_clean_failure(
        &run_cli(&["--definitely-invalid-flag"]),
        "unknown global flag",
    );
}

#[test]
fn smoke_command_matrix_matches_current_cli_surface() {
    let output = run_cli(&["--help"]);
    assert_help_output(&output, "top-level --help");
    let stdout = String::from_utf8_lossy(&output.stdout);

    let actual = parse_commands_from_top_level_help(&stdout);
    let expected = EXPECTED_COMMANDS
        .iter()
        .map(|command| command.to_string())
        .collect();

    assert_eq!(
        actual, expected,
        "smoke command matrix is out of date with current CLI surface"
    );
}

#[test]
#[named]
fn all_subcommands_bad_args_fail_cleanly() {
    setup_test!();
    let root = per_test_path!();
    let bad_invocations = expected_bad_invocations(
        per_test_path!("does-not-exist.bpt"),
        per_test_path!("does-not-exist.pkgidx"),
    );

    for (label, args) in bad_invocations {
        let output = run_cli_in_root(root, &args);
        assert_clean_failure(&output, label);
    }
}

#[test]
#[named]
fn smoke_bad_arg_matrix_matches_current_cli_surface() {
    setup_test!();

    let output = run_cli(&["--help"]);
    assert_help_output(&output, "top-level --help");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let actual = parse_commands_from_top_level_help(&stdout);

    let expected: BTreeSet<String> = expected_bad_invocations(
        per_test_path!("does-not-exist.bpt"),
        per_test_path!("does-not-exist.pkgidx"),
    )
    .into_keys()
    .map(str::to_string)
    .collect();

    assert_eq!(
        actual, expected,
        "smoke bad-argument matrix is out of date with current CLI surface"
    );
}
