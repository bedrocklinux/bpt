use crate::*;
use ::function_name::named;
use std::path::Path;

#[test]
#[named]
fn fetch_one_by_name() {
    setup_test!();

    let stdout = run!("fetch", "fakeblock").unwrap();
    assert!(stdout.contains("Fetched fakeblock@1.0.0:noarch.bpt"));
    assert!(Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
}

#[test]
#[named]
fn fetch_multiple_by_name() {
    setup_test!();

    let stdout = run!("fetch", "fakeblock", "fakeblock-song-gen").unwrap();
    assert!(stdout.contains("Fetched all 2 packages"));
    assert!(Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
    assert!(Path::new(per_test_path!("fakeblock-song-gen@1.0.0:noarch.bpt")).exists());
}

#[test]
#[named]
fn fetch_versioned_partid() {
    setup_test!();

    let stdout = run!("fetch", "fakeblock@1.0.0").unwrap();
    assert!(stdout.contains("Fetched fakeblock@1.0.0:noarch.bpt"));
    assert!(Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
}

#[test]
#[named]
fn fetch_explicit_noarch_arch() {
    setup_test!();

    let stdout = run!("fetch", "fakeblock:noarch").unwrap();
    assert!(stdout.contains("Fetched fakeblock@1.0.0:noarch.bpt"));
    assert!(Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
}

#[test]
#[named]
fn fetch_missing_pkg_errors() {
    setup_test!();

    let result = run!("fetch", "this-does-not-exist");
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Unable to locate available package"));
    assert!(stderr.contains("this-does-not-exist"));
}

#[test]
#[named]
fn fetch_dry_run_single() {
    setup_test!();

    let stdout = run!("fetch", "-D", "fakeblock").unwrap();
    assert!(stdout.contains("Would fetch"));
    assert!(stdout.contains("Dry ran fetch of 1 package(s)"));
    assert!(!Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
}

#[test]
#[named]
fn fetch_dry_run_multiple() {
    setup_test!();

    let stdout = run!("fetch", "-D", "fakeblock", "fakeblock-song-gen").unwrap();
    assert!(stdout.contains("Would fetch"));
    assert!(stdout.contains("Dry ran fetch of 2 package(s)"));
    assert!(!Path::new(per_test_path!("fakeblock@1.0.0:noarch.bpt")).exists());
    assert!(!Path::new(per_test_path!("fakeblock-song-gen@1.0.0:noarch.bpt")).exists());
}

#[test]
#[named]
fn fetch_output_exists_rejected() {
    setup_test!();

    std::fs::write(per_test_path!("fakeblock@1.0.0:noarch.bpt"), "already here").unwrap();

    let result = run!("fetch", "fakeblock");
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Unable to link file descriptor"));
    assert!(stderr.contains("fakeblock@1.0.0:noarch.bpt"));
}
