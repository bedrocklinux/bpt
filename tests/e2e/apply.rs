use crate::*;
use ::function_name::named;
use std::path::Path;

#[test]
#[named]
fn apply_world_add_installs_missing_package() {
    setup_test!();

    std::fs::create_dir_all(per_test_path!("etc/bpt")).unwrap();
    std::fs::write(per_test_path!("etc/bpt/world"), "fakeblock\n").unwrap();
    let stdout = run!("apply").unwrap();
    assert!(stdout.contains("Install"));
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("fakeblock-songs@1.0.0:noarch"));

    let installed = run!("list", "--installed").unwrap();
    assert!(installed.contains("fakeblock@1.0.0:noarch"));
    assert!(installed.contains("fakeblock-songs@1.0.0:noarch"));

    let world = std::fs::read_to_string(per_test_path!("etc/bpt/world")).unwrap();
    assert_eq!(world.trim(), "fakeblock");
}

#[test]
#[named]
fn apply_world_remove_uninstalls_no_longer_desired_packages() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();
    std::fs::create_dir_all(per_test_path!("etc/bpt")).unwrap();
    std::fs::write(per_test_path!("etc/bpt/world"), "").unwrap();

    let stdout = run!("apply").unwrap();
    assert!(stdout.contains("Remove"));
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("fakeblock-songs@1.0.0:noarch"));

    let installed = run!("list", "--installed").unwrap();
    assert!(!installed.contains("fakeblock@1.0.0:noarch"));
    assert!(!installed.contains("fakeblock-songs@1.0.0:noarch"));
    assert!(!Path::new(per_test_path!("usr/bin/fakeblock")).exists());
}

#[test]
#[named]
fn apply_dry_run_does_not_mutate_state() {
    setup_test!();

    std::fs::create_dir_all(per_test_path!("etc/bpt")).unwrap();
    std::fs::write(per_test_path!("etc/bpt/world"), "fakeblock\n").unwrap();

    let stdout = run!("apply", "-D").unwrap();
    assert!(stdout.contains("Would have:"));
    assert!(stdout.contains("Install"));
    assert!(stdout.contains("Dry ran updated installed package set"));

    let world = std::fs::read_to_string(per_test_path!("etc/bpt/world")).unwrap();
    assert_eq!(world.trim(), "fakeblock");
    assert!(
        !Path::new(per_test_path!(
            "var/lib/bpt/instpkg/fakeblock@1.0.0:noarch.instpkg"
        ))
        .exists()
    );
}
