use crate::e2e::instpkg_testutil::write_modified_bbuild;
use crate::*;
use ::function_name::named;

fn nonempty_lines(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect()
}

#[test]
#[named]
fn files_by_repository_partid() {
    setup_test!();

    let stdout = run!("files", "fakeblock").unwrap();
    let lines = nonempty_lines(&stdout);

    assert_eq!(lines.len(), 5);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("fakeblock@1.0.0:noarch"))
    );
    assert!(lines.iter().any(|line| line.contains("etc")));
    assert!(lines.iter().any(|line| line.contains("usr/bin/fakeblock")));
    assert!(lines.iter().any(|line| line.contains("etc/fakeblock.conf")));
}

#[test]
#[named]
fn files_by_bpt_path() {
    setup_test!();

    let stdout = run!("files", repo_path!("fakeblock@1.0.0:noarch.bpt")).unwrap();
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("usr/bin/fakeblock"));
    assert!(stdout.contains("etc/fakeblock.conf"));
}

#[test]
#[named]
fn files_by_bpt_url() {
    setup_test!();

    let url = repo_url!("fakeblock@1.0.0:noarch.bpt");
    let stdout = run!("files", &url).unwrap();
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("usr/bin/fakeblock"));
    assert!(stdout.contains("etc/fakeblock.conf"));
}

#[test]
#[named]
fn files_rejects_bbuild_path() {
    setup_test!();

    let result = run!("files", repo_path!("fakeblock@1.0.0.bbuild"));
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("not a valid filepath to a bpt"));
    assert!(stderr.contains("fakeblock@1.0.0.bbuild"));
}

#[test]
#[named]
fn files_rejects_bbuild_url() {
    setup_test!();

    let url = repo_url!("fakeblock@1.0.0.bbuild");
    let result = run!("files", &url);
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("not a valid http:// or https:// URL to a bpt"));
    assert!(stderr.contains("fakeblock@1.0.0.bbuild"));
}

#[test]
#[named]
fn files_missing_pkg_errors() {
    setup_test!();

    let result = run!("files", "this-does-not-exist");
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Unable to locate available package"));
    assert!(stderr.contains("this-does-not-exist"));
}

#[test]
#[named]
fn files_installed_flag_restricts_partids_to_installed_packages() {
    setup_test!();

    let result = run!("files", "--installed", "fakeblock");
    assert!(result.is_err());
    let stderr = result.unwrap_err();
    assert!(stderr.contains("Unable to locate installed package"));
    assert!(stderr.contains("fakeblock"));
}

#[test]
#[named]
fn files_by_installed_pkgid_when_repository_missing_version() {
    setup_test!();

    write_modified_bbuild(
        repo_path!("fakeblock@1.0.0.bbuild"),
        per_test_path!("fakeblock@2.0.0.bbuild"),
        &[
            ("pkgver=\"1.0.0\"", "pkgver=\"2.0.0\""),
            (
                "cat <<EOF > \"${pkgdir}/etc/fakeblock.conf\"\nsound=tok\nEOF",
                "cat <<EOF > \"${pkgdir}/etc/fakeblock.conf\"\nsound=tok\nEOF\n\nmkdir -p \"${pkgdir}/usr/share/fakeblock\"\nprintf '%s\\n' 'installed only' > \"${pkgdir}/usr/share/fakeblock/installed-only\"",
            ),
        ],
    );
    let _ = run!("install", per_test_path!("fakeblock@2.0.0.bbuild")).unwrap();

    let stdout = run!("files", "fakeblock@2.0.0").unwrap();
    assert!(stdout.contains("fakeblock@2.0.0:noarch"));
    assert!(stdout.contains("usr/bin/fakeblock"));
    assert!(stdout.contains("etc/fakeblock.conf"));
    assert!(stdout.contains("usr/share/fakeblock/installed-only"));
}

#[test]
#[named]
fn files_by_pkgid_prefers_installed_contents_over_repository_metadata() {
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

    let installed_stdout = run!("files", "fakeblock@1.0.0").unwrap();
    assert!(installed_stdout.contains("usr/share/fakeblock/installed-only"));

    let repo_stdout = run!("files", repo_path!("fakeblock@1.0.0:noarch.bpt")).unwrap();
    assert!(!repo_stdout.contains("usr/share/fakeblock/installed-only"));
    assert!(repo_stdout.contains("usr/bin/fakeblock"));
}

#[test]
#[named]
fn files_repository_flag_prefers_repository_paths_for_partids() {
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

    let stdout = run!("files", "--repository", "fakeblock@1.0.0").unwrap();
    assert!(!stdout.contains("usr/share/fakeblock/installed-only"));
    assert!(stdout.contains("usr/bin/fakeblock"));
}

#[test]
#[named]
fn files_both_flags_still_prefer_installed_paths_for_partids() {
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

    let stdout = run!("files", "--installed", "--repository", "fakeblock@1.0.0").unwrap();
    assert!(stdout.contains("usr/share/fakeblock/installed-only"));
}

#[test]
#[named]
fn files_flags_do_not_affect_path_inputs() {
    setup_test!();

    let stdout = run!(
        "files",
        "--installed",
        repo_path!("fakeblock@1.0.0:noarch.bpt")
    )
    .unwrap();
    assert!(stdout.contains("fakeblock@1.0.0:noarch"));
    assert!(stdout.contains("usr/bin/fakeblock"));
}

#[test]
#[named]
fn files_dedup_duplicate_pkg_inputs() {
    setup_test!();

    let stdout = run!(
        "files",
        "fakeblock",
        repo_path!("fakeblock@1.0.0:noarch.bpt")
    )
    .unwrap();
    let lines = nonempty_lines(&stdout);

    assert_eq!(lines.len(), 5);
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.contains("usr/bin/fakeblock"))
            .count(),
        1
    );
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.contains("etc/fakeblock.conf"))
            .count(),
        1
    );
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.contains("fakeblock@1.0.0:noarch"))
            .count(),
        5
    );
}
