use crate::e2e::instpkg_testutil::read;
use crate::*;
use ::function_name::named;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

#[test]
#[named]
fn remove_explicit_pkg_removes_orphaned_dependencies() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();
    let stdout = run!("remove", "fakeblock@1.0.0").unwrap();
    assert!(stdout.contains("Remove"));
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("fakeblock-songs@1.0.0:noarch"));

    let world = read(per_test_path!("etc/bpt/world"));
    assert!(world.trim().is_empty());
    assert!(
        !Path::new(per_test_path!(
            "var/lib/bpt/instpkg/fakeblock@1.0.0:noarch.instpkg"
        ))
        .exists()
    );
    assert!(
        !Path::new(per_test_path!(
            "var/lib/bpt/instpkg/fakeblock-songs@1.0.0:noarch.instpkg"
        ))
        .exists()
    );
    assert!(!Path::new(per_test_path!("usr/bin/fakeblock")).exists());
    assert!(!Path::new(per_test_path!("usr/share/fakeblock/songs/main-theme")).exists());
}

#[test]
#[named]
fn remove_explicit_pkg_keeps_needed_dependency_as_retain() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();
    let _ = run!("install", "fakeblock-songs").unwrap();
    let stdout = run!("remove", "fakeblock-songs").unwrap();
    assert!(stdout.contains("Retain"));
    assert!(stdout.contains("fakeblock-songs@1.0.0:noarch"));
    assert!(stdout.contains("world remove fakeblock-songs"));

    let world = read(per_test_path!("etc/bpt/world"));
    assert!(world.contains("fakeblock"));
    assert!(!world.contains("fakeblock-songs"));

    let dependency = run!("list", "--dependency").unwrap();
    assert!(dependency.contains("fakeblock-songs@1.0.0:noarch"));
}

#[test]
#[named]
fn remove_rejects_dependency_only_pkg() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();
    let result = run!("remove", "fakeblock-songs");
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("does not match any world entry"));
    assert!(stderr.contains("fakeblock-songs"));

    let world = read(per_test_path!("etc/bpt/world"));
    assert!(world.contains("fakeblock"));
    assert!(!world.contains("fakeblock-songs"));
}

#[test]
#[named]
fn remove_forget_clears_metadata_when_file_removal_fails() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();

    let usr_bin = per_test_path!("usr/bin");
    let metadata = std::fs::metadata(usr_bin).unwrap();
    let original_mode = metadata.permissions().mode();

    let mut perms = metadata.permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(usr_bin, perms).unwrap();

    let result = run!("remove", "fakeblock");

    let mut restore = std::fs::metadata(usr_bin).unwrap().permissions();
    restore.set_mode(original_mode);
    std::fs::set_permissions(usr_bin, restore).unwrap();

    let stderr = result.unwrap_err();
    assert!(stderr.contains("Unable to remove"));

    let stdout = run!("remove", "--forget", "fakeblock").unwrap();
    assert!(stdout.contains("Updated installed package set"));

    let world = read(per_test_path!("etc/bpt/world"));
    assert!(world.trim().is_empty());
    assert!(
        !Path::new(per_test_path!(
            "var/lib/bpt/instpkg/fakeblock@1.0.0:noarch.instpkg"
        ))
        .exists()
    );
    assert!(
        !Path::new(per_test_path!(
            "var/lib/bpt/instpkg/fakeblock-songs@1.0.0:noarch.instpkg"
        ))
        .exists()
    );
    assert!(Path::new(per_test_path!("usr/bin/fakeblock")).exists());
}
