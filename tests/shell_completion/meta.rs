use crate::shell_completion::common::*;
use crate::smoke::common::{
    parse_commands_from_top_level_help, parse_long_options_from_help,
    parse_short_options_from_help, run_cli,
};
use crate::*;
use ::function_name::named;
use std::collections::BTreeSet;

fn current_cli_commands() -> BTreeSet<String> {
    let output = run_cli(&["--help"]);
    assert!(
        output.status.success(),
        "top-level help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    parse_commands_from_top_level_help(&String::from_utf8_lossy(&output.stdout))
}

fn current_cli_long_options(command: &str) -> BTreeSet<String> {
    let output = run_cli(&[command, "--help"]);
    assert!(
        output.status.success(),
        "{command} help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    parse_long_options_from_help(&String::from_utf8_lossy(&output.stdout))
}

fn current_cli_short_options(command: &str) -> BTreeSet<String> {
    let output = run_cli(&[command, "--help"]);
    assert!(
        output.status.success(),
        "{command} help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    parse_short_options_from_help(&String::from_utf8_lossy(&output.stdout))
}

fn bash_top_level_commands(home: &str) -> BTreeSet<String> {
    let script = r#"
source contrib/completion/bpt.bash
COMP_WORDS=(bpt "")
COMP_CWORD=1
_bpt
printf '%s\n' "${COMPREPLY[@]}"
"#;

    assert_success(&run_shell("bash", home, script), "bash top-level commands")
        .into_iter()
        .collect()
}

fn bash_command_options(home: &str, command: &str) -> BTreeSet<String> {
    let script = format!(
        "\
source contrib/completion/bpt.bash
COMP_WORDS=(bpt {command} --)
COMP_CWORD=2
_bpt
printf '%s\\n' \"${{COMPREPLY[@]}}\"
"
    );

    assert_success(&run_shell("bash", home, &script), "bash command options")
        .into_iter()
        .filter(|line| line.starts_with("--"))
        .collect()
}

fn bash_command_short_options(home: &str, command: &str) -> BTreeSet<String> {
    let script = format!(
        "\
source contrib/completion/bpt.bash
COMP_WORDS=(bpt {command} -)
COMP_CWORD=2
_bpt
printf '%s\\n' \"${{COMPREPLY[@]}}\"
"
    );

    assert_success(
        &run_shell("bash", home, &script),
        "bash command short options",
    )
    .into_iter()
    .filter(|line| line.starts_with('-') && !line.starts_with("--"))
    .collect()
}

fn fish_top_level_commands(home: &str) -> BTreeSet<String> {
    let script = r#"
source contrib/completion/bpt.fish
complete -C "bpt "
"#;

    assert_success(&run_shell("fish", home, script), "fish top-level commands")
        .into_iter()
        .filter(|line| !line.starts_with('-'))
        .collect()
}

fn fish_command_options(home: &str, command: &str) -> BTreeSet<String> {
    let script = format!(
        "\
source contrib/completion/bpt.fish
complete -C \"bpt {command} --\"
"
    );

    assert_success(&run_shell("fish", home, &script), "fish command options")
        .into_iter()
        .filter_map(|line| line.split_whitespace().next().map(str::to_string))
        .filter(|line| line.starts_with("--"))
        .collect()
}

fn fish_command_short_options(home: &str, command: &str) -> BTreeSet<String> {
    let script = format!(
        "\
source contrib/completion/bpt.fish
complete -C \"bpt {command} -\"
"
    );

    assert_success(
        &run_shell("fish", home, &script),
        "fish command short options",
    )
    .into_iter()
    .filter_map(|line| line.split_whitespace().next().map(str::to_string))
    .filter(|line| line.starts_with('-') && !line.starts_with("--"))
    .collect()
}

fn zsh_top_level_commands(home: &str) -> BTreeSet<String> {
    let script = r#"
source contrib/completion/_bpt
_arguments() { print -r -- "$*"; }
words=(bpt "")
CURRENT=2
_bpt
"#;

    let lines = assert_success(&run_shell("zsh", home, script), "zsh top-level commands");
    let line = lines
        .into_iter()
        .find(|line| line.contains("1:command:("))
        .expect("missing zsh command spec");
    let start = line
        .find("1:command:(")
        .expect("missing zsh command spec prefix")
        + "1:command:(".len();
    let end = line[start..]
        .find(')')
        .map(|idx| start + idx)
        .expect("missing zsh command spec terminator");
    line[start..end]
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

fn zsh_command_options(home: &str, command: &str) -> BTreeSet<String> {
    let script = format!(
        "\
source contrib/completion/_bpt
_arguments() {{ print -r -- \"$*\"; }}
words=(bpt {command} --)
CURRENT=3
_bpt
"
    );

    assert_success(&run_shell("zsh", home, &script), "zsh command options")
        .join(" ")
        .split_whitespace()
        .filter_map(|token| {
            if let Some(idx) = token.find("--") {
                let option = &token[idx..];
                let end = option
                    .find(|c: char| matches!(c, '[' | ']' | '\'' | '"' | ':' | ')' | '('))
                    .unwrap_or(option.len());
                Some(option[..end].trim_end_matches('+').to_string())
            } else {
                None
            }
        })
        .collect()
}

fn zsh_command_short_options(home: &str, command: &str) -> BTreeSet<String> {
    let script = format!(
        "\
source contrib/completion/_bpt
_arguments() {{ print -r -- \"$*\"; }}
words=(bpt {command} -)
CURRENT=3
_bpt
"
    );

    assert_success(
        &run_shell("zsh", home, &script),
        "zsh command short options",
    )
    .join(" ")
    .split_whitespace()
    .filter_map(|token| {
        let normalize = |candidate: &str| {
            let candidate = candidate.trim_end_matches('+');
            if matches!(candidate, "-C" | "-g") {
                return None;
            }
            if candidate.len() == 2
                && candidate.starts_with('-')
                && candidate.as_bytes()[1].is_ascii_alphabetic()
            {
                Some(candidate.to_string())
            } else {
                None
            }
        };

        if token.starts_with('-') && !token.starts_with("--") {
            let end = token
                .find(|c: char| matches!(c, '[' | ']' | '\'' | '"' | ':' | ')' | '(' | '{'))
                .unwrap_or(token.len());
            normalize(&token[..end])
        } else if let Some(start) = token.find("(-") {
            let token = &token[start + 1..];
            let end = token
                .find(|c: char| matches!(c, '[' | ']' | '\'' | '"' | ':' | ')' | '(' | '{'))
                .unwrap_or(token.len());
            normalize(&token[..end])
        } else if let Some(start) = token.find("{-") {
            let token = &token[start + 1..];
            let end = token
                .find(|c: char| matches!(c, ',' | '}' | '[' | ']'))
                .unwrap_or(token.len());
            normalize(&token[..end])
        } else {
            None
        }
    })
    .collect()
}

#[test]
#[named]
fn shell_completion_top_level_commands_match_current_cli_surface() {
    let cli_commands = current_cli_commands();

    let bash_home = per_test_path!("bash-home");
    assert_eq!(
        bash_top_level_commands(bash_home),
        cli_commands,
        "bash top-level completion commands are out of date"
    );

    let zsh_home = per_test_path!("zsh-home");
    assert_eq!(
        zsh_top_level_commands(zsh_home),
        cli_commands,
        "zsh top-level completion commands are out of date"
    );

    let fish_home = per_test_path!("fish-home");
    assert_eq!(
        fish_top_level_commands(fish_home),
        cli_commands,
        "fish top-level completion commands are out of date"
    );
}

#[test]
#[named]
fn shell_completion_command_options_match_current_cli_surface() {
    let commands = current_cli_commands();

    let bash_home = per_test_path!("bash-home");
    let zsh_home = per_test_path!("zsh-home");
    let fish_home = per_test_path!("fish-home");

    for command in commands {
        let cli_options = current_cli_long_options(&command);
        let cli_short_options = current_cli_short_options(&command);

        assert_eq!(
            bash_command_options(bash_home, &command),
            cli_options,
            "bash completion options are out of date for `{command}`"
        );
        assert_eq!(
            zsh_command_options(zsh_home, &command),
            cli_options,
            "zsh completion options are out of date for `{command}`"
        );
        assert_eq!(
            fish_command_options(fish_home, &command),
            cli_options,
            "fish completion options are out of date for `{command}`"
        );
        assert_eq!(
            bash_command_short_options(bash_home, &command),
            cli_short_options,
            "bash completion short options are out of date for `{command}`"
        );
        assert_eq!(
            zsh_command_short_options(zsh_home, &command),
            cli_short_options,
            "zsh completion short options are out of date for `{command}`"
        );
        assert_eq!(
            fish_command_short_options(fish_home, &command),
            cli_short_options,
            "fish completion short options are out of date for `{command}`"
        );
    }
}
