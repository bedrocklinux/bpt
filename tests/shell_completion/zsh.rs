use crate::shell_completion::common::*;
use crate::*;
use ::function_name::named;

#[test]
#[named]
fn zsh_top_level_completion_includes_clean() {
    let home = per_test_path!("home");
    let script = r#"
source contrib/completion/_bpt
_arguments() { print -r -- "$*"; }
words=(bpt c)
CURRENT=2
_bpt
"#;

    let lines = assert_success(&run_shell("zsh", home, script), "zsh top-level completion");
    assert!(
        lines.iter().any(|line| line.contains(" clean ")),
        "zsh top-level completion missing clean in:\n{}",
        lines.join("\n")
    );
}

#[test]
#[named]
fn zsh_remove_completion_uses_root_world_entries() {
    let home = per_test_path!("home");
    let root_a = per_test_path!("root-a");
    let root_b = per_test_path!("root-b");
    prepare_completion_root(root_a, &["foo@1.0.0:x86_64"], &[]);
    prepare_completion_root(root_b, &["bar@2.0.0:aarch64"], &[]);

    let script = format!(
        "\
source contrib/completion/_bpt
_arguments() {{ state=world; return 0; }}
compadd() {{
    if [[ $1 == -- ]]; then
        shift
    fi
    print -l -- \"$@\"
}}
words=(bpt -R {} remove f)
CURRENT=5
_bpt
",
        shell_quote(root_a)
    );

    let lines = assert_success(
        &run_shell("zsh", home, &script),
        "zsh remove completion with -R",
    );
    assert_has_line(&lines, "foo@1.0.0:x86_64", "zsh remove completion");
    assert_lacks_prefix(&lines, "bar@2.0.0:aarch64", "zsh remove completion");
}

#[test]
#[named]
fn zsh_remove_completion_uses_bundled_root_flag_world_entries() {
    let home = per_test_path!("home");
    let root_a = per_test_path!("root-a");
    let root_b = per_test_path!("root-b");
    prepare_completion_root(root_a, &["foo@1.0.0:x86_64"], &[]);
    prepare_completion_root(root_b, &["bar@2.0.0:aarch64"], &[]);

    let script = format!(
        "\
source contrib/completion/_bpt
_arguments() {{ state=world; return 0; }}
compadd() {{
    if [[ $1 == -- ]]; then
        shift
    fi
    print -l -- \"$@\"
}}
words=(bpt -SVyR {} remove f)
CURRENT=5
_bpt
",
        shell_quote(root_a)
    );

    let lines = assert_success(
        &run_shell("zsh", home, &script),
        "zsh remove completion with bundled -R",
    );
    assert_has_line(
        &lines,
        "foo@1.0.0:x86_64",
        "zsh remove completion with bundled -R",
    );
    assert_lacks_prefix(
        &lines,
        "bar@2.0.0:aarch64",
        "zsh remove completion with bundled -R",
    );
}

#[test]
#[named]
fn zsh_check_completion_uses_root_installed_pkgids() {
    let home = per_test_path!("home");
    let root_a = per_test_path!("root-a");
    let root_b = per_test_path!("root-b");
    prepare_completion_root(root_a, &[], &["ncurses@6.6.0:x86_64"]);
    prepare_completion_root(root_b, &[], &["htop@3.4.1:x86_64"]);

    let script = format!(
        "\
source contrib/completion/_bpt
_arguments() {{ state=installed; return 0; }}
compadd() {{
    if [[ $1 == -- ]]; then
        shift
    fi
    print -l -- \"$@\"
}}
words=(bpt -R {} check n)
CURRENT=5
_bpt
",
        shell_quote(root_a)
    );

    let lines = assert_success(
        &run_shell("zsh", home, &script),
        "zsh check completion with -R",
    );
    assert_has_line(&lines, "ncurses@6.6.0:x86_64", "zsh check completion");
    assert_lacks_prefix(&lines, "htop@3.4.1:x86_64", "zsh check completion");
}

#[test]
#[named]
fn zsh_command_specs_include_recent_flags() {
    let home = per_test_path!("home");
    let script = r#"
source contrib/completion/_bpt
_arguments() { print -r -- "$*"; }

words=(bpt remove --f)
CURRENT=3
_bpt

words=(bpt sync --f)
CURRENT=3
_bpt

words=(bpt clean --)
CURRENT=3
_bpt

words=(bpt remove -)
CURRENT=3
_bpt

words=(bpt sync -)
CURRENT=3
_bpt

words=(bpt clean -)
CURRENT=3
_bpt

words=(bpt sign --n)
CURRENT=3
_bpt

words=(bpt sign -)
CURRENT=3
_bpt
"#;

    let lines = assert_success(&run_shell("zsh", home, script), "zsh option specs");
    assert!(
        lines.iter().any(|line| line.contains("--forget")),
        "zsh remove option spec missing --forget in:\n{}",
        lines.join("\n")
    );
    assert!(
        lines.iter().any(|line| line.contains("--force")),
        "zsh sync option spec missing --force in:\n{}",
        lines.join("\n")
    );
    assert!(
        lines.iter().any(|line| line.contains("--packages")),
        "zsh clean option spec missing --packages in:\n{}",
        lines.join("\n")
    );
    assert!(
        lines.iter().any(|line| line.contains("--source")),
        "zsh clean option spec missing --source in:\n{}",
        lines.join("\n")
    );
    assert!(
        lines.iter().any(|line| line.contains("--needed")),
        "zsh sign option spec missing --needed in:\n{}",
        lines.join("\n")
    );
    assert!(
        lines.iter().any(|line| line.contains("-f")),
        "zsh short option spec missing -f in:\n{}",
        lines.join("\n")
    );
    assert!(
        lines.iter().any(|line| line.contains("-n")),
        "zsh short option spec missing -n in:\n{}",
        lines.join("\n")
    );
    assert!(
        lines.iter().any(|line| line.contains("-p")),
        "zsh short option spec missing -p in:\n{}",
        lines.join("\n")
    );
    assert!(
        lines.iter().any(|line| line.contains("-s")),
        "zsh short option spec missing -s in:\n{}",
        lines.join("\n")
    );
}
