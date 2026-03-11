use crate::*;
use ::function_name::named;

#[test]
#[named]
fn list_default_includes_repository_packages() {
    setup_test!();

    let stdout = run!("list").unwrap();
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("fakeblock-songs@1.0.0:noarch"));
    assert!(stdout.contains("fakeblock-song-gen@1.0.0:noarch"));
}

#[test]
#[named]
fn list_repository_only_includes_repository_packages() {
    setup_test!();

    let stdout = run!("list", "--repository").unwrap();
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("fakeblock@1.0.0:bbuild"));
}

#[test]
#[named]
fn list_installed_only_empty_without_installs() {
    setup_test!();

    let stdout = run!("list", "--installed").unwrap();
    assert!(stdout.trim().is_empty());
}

#[test]
#[named]
fn list_explicit_or_dependency_without_installs_is_empty() {
    setup_test!();

    let explicit = run!("list", "--explicit").unwrap();
    assert!(explicit.trim().is_empty());

    let dependency = run!("list", "--dependency").unwrap();
    assert!(dependency.trim().is_empty());
}

#[test]
#[named]
fn list_installed_and_explicit_dependency_filters() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();

    let installed = run!("list", "--installed").unwrap();
    assert!(installed.contains("fakeblock@1.0.0:noarch"));
    assert!(installed.contains("fakeblock-songs@1.0.0:noarch"));

    let explicit = run!("list", "--explicit").unwrap();
    assert!(explicit.contains("fakeblock@1.0.0:noarch"));
    assert!(!explicit.contains("fakeblock-songs"));

    let dependency = run!("list", "--dependency").unwrap();
    assert!(dependency.contains("fakeblock-songs@1.0.0:noarch"));
    assert!(!dependency.contains("fakeblock@1.0.0:noarch"));
}
