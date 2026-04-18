use crate::e2e::common::bbuild::write_modified_bbuild;
use crate::*;
use ::function_name::named;

#[test]
#[named]
fn provides_default_searches_repository_files() {
    setup_test!();

    let stdout = run!("provides", "fakeblock-song-gen$").unwrap();
    assert!(stdout.contains("fakeblock-song-gen@1.0.0:noarch"));
    assert!(stdout.contains("fakeblock-song-gen"));
}

#[test]
#[named]
fn provides_repository_flag_searches_repository_files() {
    setup_test!();

    let stdout = run!("provides", "--repository", "fakeblock.conf$").unwrap();
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("fakeblock.conf"));
}

#[test]
#[named]
fn provides_installed_flag_empty_without_installs() {
    setup_test!();

    let stdout = run!("provides", "--installed", "fakeblock").unwrap();
    assert!(stdout.trim().is_empty());
}

#[test]
#[named]
fn provides_invalid_regex_errors() {
    setup_test!();

    let result = run!("provides", "(");
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Invalid regular expression"));
}

#[test]
#[named]
fn provides_installed_files() {
    setup_test!();

    let _ = run!("install", "fakeblock").unwrap();

    let stdout = run!("provides", "--installed", "fakeblock.conf$").unwrap();
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("etc/fakeblock.conf"));
    assert!(!stdout.contains("fakeblock-songs"));
}

#[test]
#[named]
fn provides_by_pkgid_prefers_installed_paths_over_repository_metadata() {
    setup_test!();

    write_modified_bbuild(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@1.0.0.bbuild"),
        &[(
            "cat <<EOF > \"${pkgdir}/etc/fakeblock.conf\"\nsound=tok\nEOF",
            "cat <<EOF > \"${pkgdir}/etc/fakeblock.conf\"\nsound=tok\nEOF\n\nmkdir -p \"${pkgdir}/usr/share/fakeblock\"\nprintf '%s\\n' 'installed only' > \"${pkgdir}/usr/share/fakeblock/installed-only\"",
        )],
    );
    let _ = run!("install", per_test_path!("fakeblock@1.0.0.bbuild")).unwrap();

    let installed_stdout = run!("provides", "installed-only$").unwrap();
    assert!(installed_stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(installed_stdout.contains("usr/share/fakeblock/installed-only"));

    let repo_stdout = run!("provides", "--repository", "installed-only$").unwrap();
    assert!(repo_stdout.trim().is_empty());
}
