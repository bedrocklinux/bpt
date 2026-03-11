use crate::shell_completion::common::*;
use crate::*;
use ::function_name::named;

#[test]
#[named]
fn bash_top_level_completion_includes_clean() {
    let home = per_test_path!("home");
    let script = r#"
source contrib/completion/bpt.bash
COMP_WORDS=(bpt c)
COMP_CWORD=1
_bpt
printf '%s\n' "${COMPREPLY[@]}"
"#;

    let lines = assert_success(
        &run_shell("bash", home, script),
        "bash top-level completion",
    );
    assert_has_line(&lines, "clean", "bash top-level completion");
}

#[test]
#[named]
fn bash_remove_completion_uses_root_world_entries() {
    let home = per_test_path!("home");
    let root_a = per_test_path!("root-a");
    let root_b = per_test_path!("root-b");
    prepare_completion_root(root_a, &["foo@1.0.0:x86_64"], &[]);
    prepare_completion_root(root_b, &["bar@2.0.0:aarch64"], &[]);

    let script = format!(
        "\
source contrib/completion/bpt.bash
COMP_WORDS=(bpt -R {} remove f)
COMP_CWORD=4
_bpt
printf '%s\\n' \"${{COMPREPLY[@]}}\"
",
        shell_quote(root_a)
    );

    let lines = assert_success(
        &run_shell("bash", home, &script),
        "bash remove completion with -R",
    );
    assert_has_line(&lines, "foo@1.0.0:x86_64", "bash remove completion");
    assert_lacks_prefix(&lines, "bar@2.0.0:aarch64", "bash remove completion");
}

#[test]
#[named]
fn bash_remove_completion_uses_bundled_root_flag_world_entries() {
    let home = per_test_path!("home");
    let root_a = per_test_path!("root-a");
    let root_b = per_test_path!("root-b");
    prepare_completion_root(root_a, &["foo@1.0.0:x86_64"], &[]);
    prepare_completion_root(root_b, &["bar@2.0.0:aarch64"], &[]);

    let script = format!(
        "\
source contrib/completion/bpt.bash
COMP_WORDS=(bpt -SVyR {} remove f)
COMP_CWORD=4
_bpt
printf '%s\\n' \"${{COMPREPLY[@]}}\"
",
        shell_quote(root_a)
    );

    let lines = assert_success(
        &run_shell("bash", home, &script),
        "bash remove completion with bundled -R",
    );
    assert_has_line(
        &lines,
        "foo@1.0.0:x86_64",
        "bash remove completion with bundled -R",
    );
    assert_lacks_prefix(
        &lines,
        "bar@2.0.0:aarch64",
        "bash remove completion with bundled -R",
    );
}

#[test]
#[named]
fn bash_check_completion_uses_root_installed_pkgids() {
    let home = per_test_path!("home");
    let root_a = per_test_path!("root-a");
    let root_b = per_test_path!("root-b");
    prepare_completion_root(root_a, &[], &["ncurses@6.6.0:x86_64"]);
    prepare_completion_root(root_b, &[], &["htop@3.4.1:x86_64"]);

    let script = format!(
        "\
source contrib/completion/bpt.bash
COMP_WORDS=(bpt -R {} check n)
COMP_CWORD=4
_bpt
printf '%s\\n' \"${{COMPREPLY[@]}}\"
",
        shell_quote(root_a)
    );

    let lines = assert_success(
        &run_shell("bash", home, &script),
        "bash check completion with -R",
    );
    assert_has_line(&lines, "ncurses@6.6.0:x86_64", "bash check completion");
    assert_lacks_prefix(&lines, "htop@3.4.1:x86_64", "bash check completion");
}

#[test]
#[named]
fn bash_command_options_include_recent_flags() {
    let home = per_test_path!("home");
    let script = r#"
source contrib/completion/bpt.bash

COMP_WORDS=(bpt remove --f)
COMP_CWORD=2
_bpt
printf 'remove:%s\n' "${COMPREPLY[@]}"

COMP_WORDS=(bpt sync --f)
COMP_CWORD=2
_bpt
printf 'sync:%s\n' "${COMPREPLY[@]}"

COMP_WORDS=(bpt clean --)
COMP_CWORD=2
_bpt
printf 'clean:%s\n' "${COMPREPLY[@]}"

COMP_WORDS=(bpt remove -)
COMP_CWORD=2
_bpt
printf 'remove-short:%s\n' "${COMPREPLY[@]}"

COMP_WORDS=(bpt sync -)
COMP_CWORD=2
_bpt
printf 'sync-short:%s\n' "${COMPREPLY[@]}"

COMP_WORDS=(bpt clean -)
COMP_CWORD=2
_bpt
printf 'clean-short:%s\n' "${COMPREPLY[@]}"

COMP_WORDS=(bpt sign --n)
COMP_CWORD=2
_bpt
printf 'sign:%s\n' "${COMPREPLY[@]}"

COMP_WORDS=(bpt sign -)
COMP_CWORD=2
_bpt
printf 'sign-short:%s\n' "${COMPREPLY[@]}"
"#;

    let lines = assert_success(&run_shell("bash", home, script), "bash option completion");
    assert_has_line(&lines, "remove:--forget", "bash remove option completion");
    assert_has_line(&lines, "sync:--force", "bash sync option completion");
    assert_has_line(&lines, "clean:--packages", "bash clean option completion");
    assert_has_line(&lines, "clean:--source", "bash clean option completion");
    assert_has_line(
        &lines,
        "remove-short:-f",
        "bash remove short option completion",
    );
    assert_has_line(&lines, "sync-short:-f", "bash sync short option completion");
    assert_has_line(
        &lines,
        "clean-short:-p",
        "bash clean short option completion",
    );
    assert_has_line(
        &lines,
        "clean-short:-s",
        "bash clean short option completion",
    );
    assert_has_line(&lines, "sign:--needed", "bash sign option completion");
    assert_has_line(&lines, "sign-short:-n", "bash sign short option completion");
}
