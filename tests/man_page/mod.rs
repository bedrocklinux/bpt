use crate::smoke::common::{
    parse_commands_from_top_level_help, parse_long_options_from_help,
    parse_short_options_from_help, run_cli,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::{Command, Stdio};

fn man_source() -> &'static str {
    include_str!("../../doc/man/bpt.1.scd")
}

fn conf_man_source() -> &'static str {
    include_str!("../../doc/man/bpt.conf.5.scd")
}

fn current_cli_commands() -> BTreeSet<String> {
    let output = run_cli(&["--help"]);
    assert!(
        output.status.success(),
        "top-level help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    parse_commands_from_top_level_help(&String::from_utf8_lossy(&output.stdout))
}

fn current_common_options() -> BTreeSet<String> {
    let output = run_cli(&["--help"]);
    assert!(
        output.status.success(),
        "top-level help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut options = parse_long_options_from_help(&stdout);
    options.extend(parse_short_options_from_help(&stdout));
    options
}

fn current_command_specific_options(command: &str) -> BTreeSet<String> {
    let output = run_cli(&[command, "--help"]);
    assert!(
        output.status.success(),
        "{command} help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut options = parse_long_options_from_help(&stdout);
    options.extend(parse_short_options_from_help(&stdout));

    let common = current_common_options();
    options.retain(|opt| !common.contains(opt));
    options
}

fn emphasized_spans(line: &str) -> impl Iterator<Item = &str> {
    line.split('*').skip(1).step_by(2)
}

fn extract_options(line: &str) -> Vec<String> {
    emphasized_spans(line)
        .filter_map(|span| {
            if !span.starts_with('-') {
                return None;
            }
            let option = span.split('=').next().unwrap_or(span).trim();
            if option.is_empty() {
                None
            } else {
                Some(option.to_string())
            }
        })
        .collect()
}

fn man_common_options() -> BTreeSet<String> {
    let mut in_section = false;
    let mut options = BTreeSet::new();

    for line in man_source().lines() {
        if line == "# COMMON OPTIONS" {
            in_section = true;
            continue;
        }
        if in_section && line.starts_with("# ") {
            break;
        }
        if in_section {
            options.extend(extract_options(line));
        }
    }

    options
}

fn man_command_sections() -> BTreeSet<String> {
    let mut in_commands = false;
    let mut commands = BTreeSet::new();

    for line in man_source().lines() {
        if line == "# COMMANDS" {
            in_commands = true;
            continue;
        }
        if in_commands && line.starts_with("# ") {
            break;
        }
        if let Some(command) = line.strip_prefix("## ") {
            commands.insert(command.to_string());
        }
    }

    commands
}

fn man_command_options() -> BTreeMap<String, BTreeSet<String>> {
    let mut in_commands = false;
    let mut current_command: Option<String> = None;
    let mut options = BTreeMap::<String, BTreeSet<String>>::new();

    for line in man_source().lines() {
        if line == "# COMMANDS" {
            in_commands = true;
            continue;
        }
        if in_commands && line.starts_with("# ") {
            break;
        }
        if let Some(command) = line.strip_prefix("## ") {
            current_command = Some(command.to_string());
            options.entry(command.to_string()).or_default();
            continue;
        }
        if let Some(command) = &current_command {
            options
                .entry(command.clone())
                .or_default()
                .extend(extract_options(line));
        }
    }

    options
}

fn config_sections_from_default_conf() -> BTreeSet<String> {
    include_str!("../../assets/default-configs/bpt.conf")
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                Some(line.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn config_keys_from_default_conf() -> BTreeMap<String, BTreeSet<String>> {
    let mut section: Option<String> = None;
    let mut keys = BTreeMap::<String, BTreeSet<String>>::new();

    for line in include_str!("../../assets/default-configs/bpt.conf").lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = Some(line.to_string());
            keys.entry(line.to_string()).or_default();
            continue;
        }
        let Some(current) = &section else {
            continue;
        };
        if let Some((key, _)) = line.split_once('=') {
            keys.entry(current.clone())
                .or_default()
                .insert(key.trim().to_string());
        }
    }

    keys
}

fn conf_man_sections() -> BTreeSet<String> {
    conf_man_source()
        .lines()
        .filter_map(|line| line.strip_prefix("## ").map(str::to_string))
        .collect()
}

fn conf_man_keys() -> BTreeMap<String, BTreeSet<String>> {
    let mut section: Option<String> = None;
    let mut keys = BTreeMap::<String, BTreeSet<String>>::new();

    for line in conf_man_source().lines() {
        if line.starts_with("# ") {
            section = None;
        }
        if let Some(name) = line.strip_prefix("## ") {
            section = Some(name.to_string());
            keys.entry(name.to_string()).or_default();
            continue;
        }
        if let Some(section) = &section {
            let parts = line.split('*').collect::<Vec<_>>();
            if parts.len() >= 3
                && parts[0].is_empty()
                && parts[2].is_empty()
                && !parts[1].contains(' ')
                && !parts[1].contains('=')
            {
                keys.entry(section.clone())
                    .or_default()
                    .insert(parts[1].to_string());
            }
        }
    }

    keys
}

fn see_also_targets(man: &str) -> Vec<(String, String)> {
    let mut in_section = false;
    let mut refs = Vec::new();

    for line in man.lines() {
        if line == "# SEE ALSO" {
            in_section = true;
            continue;
        }
        if in_section && line.starts_with("# ") {
            break;
        }
        if !in_section {
            continue;
        }

        let parts = line.split('*').collect::<Vec<_>>();
        for span in parts.iter().skip(1).step_by(2) {
            if let Some(section) = parts
                .windows(2)
                .find_map(|window| (window[0] == *span).then_some(window[1]))
                .and_then(|suffix| suffix.strip_prefix('('))
                .map(|suffix| suffix.trim_end_matches(')'))
            {
                refs.push((span.to_string(), section.to_string()));
            }
        }
    }

    refs
}

fn rendered_man_page(path: &str) -> String {
    let source = std::fs::read_to_string(path).expect("failed to read man page source");
    let mut child = Command::new("scdoc")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn scdoc");

    use std::io::Write;
    child
        .stdin
        .as_mut()
        .expect("missing scdoc stdin")
        .write_all(source.as_bytes())
        .expect("failed to write man page source to scdoc");

    let output = child
        .wait_with_output()
        .expect("failed to wait for scdoc output");
    assert!(
        output.status.success(),
        "scdoc failed for `{path}`: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("scdoc output was not UTF-8")
}

#[test]
fn man_page_command_list_matches_current_cli_surface() {
    assert_eq!(
        man_command_sections(),
        current_cli_commands(),
        "man page command list is out of date with current CLI surface"
    );
}

#[test]
fn man_page_common_options_match_current_cli_surface() {
    assert_eq!(
        man_common_options(),
        current_common_options(),
        "man page common options are out of date with current CLI surface"
    );
}

#[test]
fn man_page_command_options_match_current_cli_surface() {
    let man_options = man_command_options();

    for command in current_cli_commands() {
        let actual = current_command_specific_options(&command);
        let documented = man_options.get(&command).cloned().unwrap_or_default();
        assert_eq!(
            documented, actual,
            "man page options are out of date for `{command}`"
        );
    }
}

#[test]
fn bpt_conf_man_page_sections_match_default_config() {
    assert_eq!(
        conf_man_sections(),
        config_sections_from_default_conf(),
        "bpt.conf man page sections are out of date with the shipped default config"
    );
}

#[test]
fn bpt_conf_man_page_keys_match_default_config() {
    assert_eq!(
        conf_man_keys(),
        config_keys_from_default_conf(),
        "bpt.conf man page keys are out of date with the shipped default config"
    );
}

#[test]
fn man_page_see_also_targets_exist() {
    for (name, section) in see_also_targets(man_source())
        .into_iter()
        .chain(see_also_targets(conf_man_source()))
    {
        let path = format!("doc/man/{name}.{section}.scd");
        assert!(
            Path::new(&path).exists(),
            "SEE ALSO target `{name}({section})` is missing source file `{path}`"
        );
    }
}

#[test]
fn rendered_bpt_man_page_matches_source() {
    assert_eq!(
        std::fs::read_to_string("doc/man/bpt.1").expect("failed to read rendered man page"),
        rendered_man_page("doc/man/bpt.1.scd"),
        "doc/man/bpt.1 is out of date with doc/man/bpt.1.scd"
    );
}

#[test]
fn rendered_bpt_conf_man_page_matches_source() {
    assert_eq!(
        std::fs::read_to_string("doc/man/bpt.conf.5").expect("failed to read rendered man page"),
        rendered_man_page("doc/man/bpt.conf.5.scd"),
        "doc/man/bpt.conf.5 is out of date with doc/man/bpt.conf.5.scd"
    );
}
