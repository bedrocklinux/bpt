use std::fs;
use std::path::Path;
use std::process::Command;

pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

pub fn prepare_completion_root(root: &str, world_entries: &[&str], installed_pkgids: &[&str]) {
    let etc_bpt = Path::new(root).join("etc/bpt");
    let instpkg_dir = Path::new(root).join("var/lib/bpt/instpkg");
    fs::create_dir_all(&etc_bpt).unwrap();
    fs::create_dir_all(&instpkg_dir).unwrap();

    let world = if world_entries.is_empty() {
        String::new()
    } else {
        format!("{}\n", world_entries.join("\n"))
    };
    fs::write(etc_bpt.join("world"), world).unwrap();

    for pkgid in installed_pkgids {
        fs::write(instpkg_dir.join(format!("{pkgid}.instpkg")), "").unwrap();
    }
}

pub fn run_shell(shell: &str, home: &str, script: &str) -> std::process::Output {
    let bin_path = Path::new(env!("CARGO_BIN_EXE_bpt"))
        .parent()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let path = std::env::var("PATH").unwrap_or_default();

    fs::create_dir_all(home).unwrap();
    fs::create_dir_all(format!("{home}/.config")).unwrap();
    fs::create_dir_all(format!("{home}/.local/share")).unwrap();

    let mut cmd = Command::new(shell);
    match shell {
        "bash" => {
            cmd.args(["--noprofile", "--norc", "-c", script]);
        }
        "zsh" => {
            cmd.args(["-fc", script]);
        }
        "fish" => {
            cmd.args(["-c", script]);
        }
        _ => panic!("unsupported shell: {shell}"),
    }

    cmd.env("HOME", home)
        .env("XDG_CONFIG_HOME", format!("{home}/.config"))
        .env("XDG_DATA_HOME", format!("{home}/.local/share"))
        .env("XDG_CACHE_HOME", format!("{home}/.cache"))
        .env("ZDOTDIR", home)
        .env("PATH", format!("{bin_path}:{path}"))
        .output()
        .expect("failed to execute shell")
}

pub fn assert_success(output: &std::process::Output, label: &str) -> Vec<String> {
    assert!(
        output.status.success(),
        "{label} failed:\nstdout:{}\nstderr:{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn assert_has_line(lines: &[String], expected: &str, label: &str) {
    assert!(
        lines
            .iter()
            .any(|line| line == expected || line.starts_with(&format!("{expected}\t"))),
        "{label} missing `{expected}` in:\n{}",
        lines.join("\n")
    );
}

pub fn assert_lacks_prefix(lines: &[String], unexpected: &str, label: &str) {
    assert!(
        !lines
            .iter()
            .any(|line| line == unexpected || line.starts_with(&format!("{unexpected}\t"))),
        "{label} unexpectedly contained `{unexpected}` in:\n{}",
        lines.join("\n")
    );
}
