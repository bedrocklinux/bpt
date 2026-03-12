use crate::e2e::instpkg_testutil::{read, write_modified_bbuild};
use crate::*;
use ::function_name::named;

#[test]
#[named]
fn upgrade_path_bbuild_replaces_installed_version_and_updates_world_entry() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();
    write_modified_bbuild(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@2.0.0.bbuild"),
        &[
            ("pkgver=\"1.0.0\"", "pkgver=\"2.0.0\""),
            (
                "pkgdesc=\"just a boolean driven aggregation, really, of what programmers call hacker traps\"",
                "pkgdesc=\"version two fakeblock\"",
            ),
            ("sound=tok", "sound=bip"),
        ],
    );

    let stdout = run!("upgrade", per_test_path!("fakeblock@2.0.0.bbuild")).unwrap();
    assert!(stdout.contains("Upgrade"));
    assert!(stdout.contains("fakeblock@2.0.0:noarch"));
    assert!(stdout.contains("from fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("build"));

    let world = read(per_test_path!("etc/bpt/world"));
    assert!(world.contains("fakeblock:noarch"));
    assert!(!world.contains("fakeblock@2.0.0"));

    let info = run!("info", "fakeblock@2.0.0").unwrap();
    assert!(info.contains("Name:         fakeblock"));
    assert!(info.contains("Version:      2.0.0"));
    assert!(info.contains("Architecture: noarch"));
    assert!(info.contains("version two fakeblock"));
    assert!(read(per_test_path!("etc/fakeblock.conf")).contains("sound=bip"));
}

#[test]
#[named]
fn upgrade_dependency_only_pkg_allowed_without_world_change() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();
    write_modified_bbuild(
        repo_path!("fakeblock-songs@1.0.0.bbuild"),
        per_test_path!("fakeblock-songs@2.0.0.bbuild"),
        &[
            ("pkgver=\"1.0.0\"", "pkgver=\"2.0.0\""),
            (
                "pkgdesc=\"dependency test package\"",
                "pkgdesc=\"dependency test package version two\"",
            ),
        ],
    );

    let stdout = run!("upgrade", per_test_path!("fakeblock-songs@2.0.0.bbuild")).unwrap();
    assert!(stdout.contains("Upgrade"));
    assert!(stdout.contains("fakeblock-songs@2.0.0:noarch"));
    assert!(stdout.contains("from fakeblock-songs@1.0.0:noarch"));
    assert!(!stdout.contains("world"));

    let world = read(per_test_path!("etc/bpt/world"));
    assert_eq!(world.trim(), "fakeblock");

    let explicit = run!("list", "--explicit").unwrap();
    assert!(explicit.contains("fakeblock@1.0.0:noarch"));
    assert!(!explicit.contains("fakeblock-songs"));

    let dependency = run!("list", "--dependency").unwrap();
    assert!(dependency.contains("fakeblock-songs@2.0.0:noarch"));
}

#[test]
#[named]
fn upgrade_all_no_args_is_noop_at_latest_repo_versions() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();
    let stdout = run!("upgrade").unwrap();
    assert!(stdout.contains("No changes needed"));
}

#[test]
#[named]
fn upgrade_repository_partid_uses_latest_repository_version() {
    setup_test!();

    write_modified_bbuild(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@0.9.0.bbuild"),
        &[
            ("pkgver=\"1.0.0\"", "pkgver=\"0.9.0\""),
            (
                "pkgdesc=\"just a boolean driven aggregation, really, of what programmers call hacker traps\"",
                "pkgdesc=\"version zero point nine fakeblock\"",
            ),
        ],
    );

    let _ = run!("install", per_test_path!("fakeblock@0.9.0.bbuild")).unwrap();
    let stdout = run!("upgrade", "fakeblock").unwrap();
    assert!(stdout.contains("Upgrade"));
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("from fakeblock@0.9.0:noarch"));

    let world = read(per_test_path!("etc/bpt/world"));
    assert_eq!(world.trim(), "fakeblock");

    let explicit = run!("list", "--explicit").unwrap();
    assert!(explicit.contains("fakeblock@1.0.0:noarch"));
    assert!(!explicit.contains("fakeblock@0.9.0:noarch"));
}

#[test]
#[named]
fn upgrade_removes_stale_empty_directories() {
    setup_test!();

    std::fs::write(
        per_test_path!("compmove@1.0.0.bbuild"),
        r#"#!/bin/sh
pkgname="compmove"
pkgver="1.0.0"
pkgdesc="completion path move test"
homepage="N/A"
license="MIT"
depends=""
backup=""
makearch="noarch"
makedepends=""
source=""
sha256sums=""

build() {
    mkdir -p "${pkgdir}/usr/share/zsh/site-functions"
    printf '%s\n' '#compdef compmove' > "${pkgdir}/usr/share/zsh/site-functions/_compmove"
}
"#,
    )
    .unwrap();
    std::fs::write(
        per_test_path!("compmove@2.0.0.bbuild"),
        r#"#!/bin/sh
pkgname="compmove"
pkgver="2.0.0"
pkgdesc="completion path move test"
homepage="N/A"
license="MIT"
depends=""
backup=""
makearch="noarch"
makedepends=""
source=""
sha256sums=""

build() {
    mkdir -p "${pkgdir}/usr/local/share/zsh/site-functions"
    printf '%s\n' '#compdef compmove' > "${pkgdir}/usr/local/share/zsh/site-functions/_compmove"
}
"#,
    )
    .unwrap();

    let _ = run!("install", per_test_path!("compmove@1.0.0.bbuild")).unwrap();
    assert!(
        std::path::Path::new(per_test_path!("usr/share/zsh/site-functions/_compmove")).exists()
    );

    let stdout = run!("upgrade", per_test_path!("compmove@2.0.0.bbuild")).unwrap();
    assert!(stdout.contains("Upgrade"));
    assert!(stdout.contains("compmove@2.0.0:noarch"));
    assert!(stdout.contains("from compmove@1.0.0:noarch"));

    assert!(!std::path::Path::new(per_test_path!("usr/share/zsh/site-functions")).exists());
    assert!(
        std::path::Path::new(per_test_path!(
            "usr/local/share/zsh/site-functions/_compmove"
        ))
        .exists()
    );
}
