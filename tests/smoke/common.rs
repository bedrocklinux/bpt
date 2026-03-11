use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

pub(crate) const EXPECTED_COMMANDS: &[&str] = &[
    "install",
    "remove",
    "upgrade",
    "downgrade",
    "apply",
    "check",
    "info",
    "files",
    "search",
    "list",
    "provides",
    "sync",
    "fetch",
    "clean",
    "build",
    "make-repo",
    "verify",
    "sign",
];

pub(crate) fn expected_bad_invocations<'a>(
    missing_path: &'a str,
    missing_index: &'a str,
) -> BTreeMap<&'static str, Vec<&'a str>> {
    BTreeMap::from([
        ("install", vec!["install", "this-does-not-exist"]),
        ("remove", vec!["remove", "this-does-not-exist"]),
        ("upgrade", vec!["upgrade", "this-does-not-exist"]),
        ("downgrade", vec!["downgrade", "this-does-not-exist"]),
        ("apply", vec!["apply", "--definitely-invalid-flag"]),
        ("check", vec!["check", "this-does-not-exist"]),
        ("info", vec!["info", "this-does-not-exist"]),
        ("files", vec!["files", "this-does-not-exist"]),
        ("search", vec!["search", "("]),
        ("list", vec!["list", "--definitely-invalid-flag"]),
        ("provides", vec!["provides", "("]),
        ("sync", vec!["sync", missing_index]),
        ("fetch", vec!["fetch", "this-does-not-exist"]),
        ("clean", vec!["clean", "--definitely-invalid-flag"]),
        ("build", vec!["build", "this-does-not-exist"]),
        ("make-repo", vec!["make-repo", "--definitely-invalid-flag"]),
        ("verify", vec!["verify", missing_path]),
        ("sign", vec!["sign", missing_path]),
    ])
}

pub(crate) fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_bpt"))
        .args(args)
        .output()
        .expect("failed to execute bpt")
}

pub(crate) fn parse_commands_from_top_level_help(help: &str) -> BTreeSet<String> {
    let mut commands = BTreeSet::new();
    let mut in_commands = false;

    for line in help.lines() {
        if line == "Commands:" {
            in_commands = true;
            continue;
        }
        if !in_commands {
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        if !line.starts_with("  ") {
            break;
        }

        if let Some(command) = line.split_whitespace().next() {
            commands.insert(command.to_string());
        }
    }

    commands
}

pub(crate) fn parse_long_options_from_help(help: &str) -> BTreeSet<String> {
    let mut options = BTreeSet::new();

    for line in help.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('-') {
            continue;
        }

        for part in trimmed.split(',') {
            let token = part.split_whitespace().next().unwrap_or_default();
            if token.starts_with("--") {
                options.insert(token.to_string());
            }
        }
    }

    options
}

pub(crate) fn parse_short_options_from_help(help: &str) -> BTreeSet<String> {
    let mut options = BTreeSet::new();

    for line in help.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('-') {
            continue;
        }

        for part in trimmed.split(',') {
            let token = part.split_whitespace().next().unwrap_or_default();
            if token.len() == 2
                && token.starts_with('-')
                && !token.starts_with("--")
                && token.as_bytes()[1].is_ascii_alphabetic()
            {
                options.insert(token.to_string());
            }
        }
    }

    options
}
