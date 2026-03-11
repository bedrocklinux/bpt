use crate::shell_completion::common::*;
use crate::*;
use ::function_name::named;

#[test]
#[named]
fn fish_top_level_completion_includes_clean() {
    let home = per_test_path!("home");
    let script = r#"
source contrib/completion/bpt.fish
complete -C "bpt c"
"#;

    let lines = assert_success(
        &run_shell("fish", home, script),
        "fish top-level completion",
    );
    assert_has_line(&lines, "clean", "fish top-level completion");
}

#[test]
#[named]
fn fish_remove_completion_uses_root_world_entries() {
    let home = per_test_path!("home");
    let root_a = per_test_path!("root-a");
    let root_b = per_test_path!("root-b");
    prepare_completion_root(root_a, &["foo@1.0.0:x86_64"], &[]);
    prepare_completion_root(root_b, &["bar@2.0.0:aarch64"], &[]);

    let script = format!(
        "\
source contrib/completion/bpt.fish
complete -C \"bpt -R {} remove f\"
",
        root_a
    );

    let lines = assert_success(
        &run_shell("fish", home, &script),
        "fish remove completion with -R",
    );
    assert_has_line(&lines, "foo@1.0.0:x86_64", "fish remove completion");
    assert_lacks_prefix(&lines, "bar@2.0.0:aarch64", "fish remove completion");
}

#[test]
#[named]
fn fish_remove_completion_uses_bundled_root_flag_world_entries() {
    let home = per_test_path!("home");
    let root_a = per_test_path!("root-a");
    let root_b = per_test_path!("root-b");
    prepare_completion_root(root_a, &["foo@1.0.0:x86_64"], &[]);
    prepare_completion_root(root_b, &["bar@2.0.0:aarch64"], &[]);

    let script = format!(
        "\
source contrib/completion/bpt.fish
complete -C \"bpt -SVyR {} remove f\"
",
        root_a
    );

    let lines = assert_success(
        &run_shell("fish", home, &script),
        "fish remove completion with bundled -R",
    );
    assert_has_line(
        &lines,
        "foo@1.0.0:x86_64",
        "fish remove completion with bundled -R",
    );
    assert_lacks_prefix(
        &lines,
        "bar@2.0.0:aarch64",
        "fish remove completion with bundled -R",
    );
}

#[test]
#[named]
fn fish_check_completion_uses_root_installed_pkgids() {
    let home = per_test_path!("home");
    let root_a = per_test_path!("root-a");
    let root_b = per_test_path!("root-b");
    prepare_completion_root(root_a, &[], &["ncurses@6.6.0:x86_64"]);
    prepare_completion_root(root_b, &[], &["htop@3.4.1:x86_64"]);

    let script = format!(
        "\
source contrib/completion/bpt.fish
complete -C \"bpt -R {} check n\"
",
        root_a
    );

    let lines = assert_success(
        &run_shell("fish", home, &script),
        "fish check completion with -R",
    );
    assert_has_line(&lines, "ncurses@6.6.0:x86_64", "fish check completion");
    assert_lacks_prefix(&lines, "htop@3.4.1:x86_64", "fish check completion");
}

#[test]
#[named]
fn fish_command_options_include_recent_flags() {
    let home = per_test_path!("home");
    let script = r#"
source contrib/completion/bpt.fish
complete -C "bpt remove --f"
complete -C "bpt sync --f"
complete -C "bpt clean --"
complete -C "bpt remove -"
complete -C "bpt sync -"
complete -C "bpt clean -"
complete -C "bpt sign --n"
complete -C "bpt sign -"
"#;

    let lines = assert_success(&run_shell("fish", home, script), "fish option completion");
    assert_has_line(&lines, "--forget", "fish remove option completion");
    assert_has_line(&lines, "--force", "fish sync option completion");
    assert_has_line(&lines, "--packages", "fish clean option completion");
    assert_has_line(&lines, "--source", "fish clean option completion");
    assert_has_line(&lines, "--needed", "fish sign option completion");
    assert_has_line(&lines, "-f", "fish remove short option completion");
    assert_has_line(&lines, "-f", "fish sync short option completion");
    assert_has_line(&lines, "-n", "fish sign short option completion");
    assert_has_line(&lines, "-p", "fish clean short option completion");
    assert_has_line(&lines, "-s", "fish clean short option completion");
}
