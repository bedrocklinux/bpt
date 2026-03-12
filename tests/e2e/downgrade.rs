use crate::e2e::instpkg_testutil::{read, write_modified_bbuild};
use crate::*;
use ::function_name::named;

#[test]
#[named]
fn downgrade_repository_partid_with_version_replaces_installed_version_and_world_entry() {
    setup_test!();

    write_modified_bbuild(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@2.0.0.bbuild"),
        &[("pkgver=\"1.0.0\"", "pkgver=\"2.0.0\"")],
    );

    let _ = run!("install", per_test_path!("fakeblock@2.0.0.bbuild")).unwrap();
    let stdout = run!("downgrade", "fakeblock@1.0.0").unwrap();
    assert!(stdout.contains("Downgrade"));
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("from fakeblock@2.0.0:noarch"));

    let world = read(per_test_path!("etc/bpt/world"));
    assert_eq!(world.trim(), "fakeblock@1.0.0");

    let explicit = run!("list", "--explicit").unwrap();
    assert!(explicit.contains("fakeblock@1.0.0:noarch"));
    assert!(!explicit.contains("fakeblock@2.0.0:noarch"));
}

#[test]
#[named]
fn downgrade_dependency_only_pkg_rejected() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();
    write_modified_bbuild(
        repo_path!("fakeblock-songs@1.0.0.bbuild"),
        per_test_path!("fakeblock-songs@2.0.0.bbuild"),
        &[("pkgver=\"1.0.0\"", "pkgver=\"2.0.0\"")],
    );
    let _ = run!("upgrade", per_test_path!("fakeblock-songs@2.0.0.bbuild")).unwrap();

    let result = run!("downgrade", "fakeblock-songs@1.0.0");
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Cannot downgrade dependency-only package"));
    assert!(stderr.contains("fakeblock-songs@1.0.0"));

    let world = read(per_test_path!("etc/bpt/world"));
    assert_eq!(world.trim(), "fakeblock");
}

#[test]
#[named]
fn downgrade_repo_partid_without_version_errors() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();
    let result = run!("downgrade", "fakeblock");
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("missing version"));
    assert!(stderr.contains("fakeblock"));
}
