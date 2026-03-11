use crate::*;
use ::function_name::named;

#[test]
#[named]
fn search_default_matches_package_name() {
    setup_test!();

    let stdout = run!("search", "^fakeblock-song-gen$").unwrap();
    assert!(stdout.contains("fakeblock-song-gen@1.0.0:noarch"));
}

#[test]
#[named]
fn search_description_flag_matches_description_only() {
    setup_test!();

    let stdout = run!("search", "--description", "boolean driven aggregation").unwrap();
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("boolean driven aggregation"));
}

#[test]
#[named]
fn search_name_flag_matches_name_only() {
    setup_test!();

    let stdout = run!("search", "--name", "^fakeblock-songs$").unwrap();
    assert!(stdout.contains("fakeblock-songs@1.0.0:noarch"));
    assert!(!stdout.contains("make dependency test package"));
}

#[test]
#[named]
fn search_installed_flag_empty_without_installs() {
    setup_test!();

    let stdout = run!("search", "--installed", "fakeblock").unwrap();
    assert!(stdout.trim().is_empty());
}

#[test]
#[named]
fn search_invalid_regex_errors() {
    setup_test!();

    let result = run!("search", "(");
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Invalid regular expression"));
}

#[test]
#[named]
fn search_installed_packages() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();

    let stdout = run!("search", "--installed", "fakeblock-song").unwrap();
    assert!(stdout.contains("fakeblock-songs@1.0.0:noarch"));
    assert!(stdout.contains("dependency test package"));
    assert!(!stdout.contains(":bbuild"));
}
