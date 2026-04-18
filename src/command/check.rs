use crate::{cli::*, collection::*, color::Color, error::*, file::*, metadata::*};

#[derive(Clone, Copy)]
enum BackupCheckMode {
    Warn,
    Strict,
    Ignore,
}

pub fn check(
    flags: CommonFlags,
    pkgs: Vec<PartId>,
    strict: bool,
    ignore_backup: bool,
) -> Result<String, Err> {
    let backup_mode = match (strict, ignore_backup) {
        (true, true) => {
            return Err(Err::InputFieldInvalid(
                "check options",
                "--strict and --ignore-backup cannot be used together".to_string(),
            ));
        }
        (true, false) => BackupCheckMode::Strict,
        (false, true) => BackupCheckMode::Ignore,
        (false, false) => BackupCheckMode::Warn,
    };

    let bpt_conf = &BptConf::from_root_path(&flags.root_dir)?;
    let installed_pkgs = &InstalledPkgs::from_root_path_ro(&flags.root_dir)?;
    let archs = &bpt_conf.general.default_archs;

    let mut targets = Vec::new();
    if pkgs.is_empty() {
        targets.extend(installed_pkgs.pkgids().cloned());
    } else {
        for partid in pkgs {
            let pkgid = installed_pkgs
                .best_match(&partid, archs)
                .map(|instpkg| instpkg.pkgid().clone())
                .ok_or_else(|| Err::UnableToLocateInstalledPkg(partid.clone()))?;
            targets.push(pkgid);
        }
    }

    if targets.is_empty() {
        return Ok("No installed packages to check".to_owned());
    }

    targets.sort();
    targets.dedup();

    let mut error_issues_by_pkg = Vec::new();
    let mut warning_issues_by_pkg = Vec::new();
    for pkgid in &targets {
        let instpkg = installed_pkgs
            .get(pkgid)
            .expect("selected installed package disappeared during check");
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        for issue in instpkg.check(flags.root_dir.as_path())? {
            match issue {
                InstPkgCheckIssue::Error(message) => errors.push(message),
                InstPkgCheckIssue::BackupDiff(message) => match backup_mode {
                    BackupCheckMode::Warn => warnings.push(message),
                    BackupCheckMode::Strict => errors.push(message),
                    BackupCheckMode::Ignore => {}
                },
            }
        }
        if !errors.is_empty() {
            error_issues_by_pkg.push((pkgid.color().to_string(), errors));
        }
        if !warnings.is_empty() {
            warning_issues_by_pkg.push((pkgid.color().to_string(), warnings));
        }
    }

    if !warning_issues_by_pkg.is_empty() {
        println!(
            "{}Warning:{} Installed backup file differences:\n{}",
            Color::Warn,
            Color::Default,
            format_check_report(&warning_issues_by_pkg),
        );
    }

    if !error_issues_by_pkg.is_empty() {
        return Err(Err::CheckFailed(format_check_report(&error_issues_by_pkg)));
    }

    if targets.len() == 1 {
        Ok(format!("Checked installed package {}", targets[0]))
    } else {
        Ok(format!("Checked all {} installed packages", targets.len()))
    }
}

fn format_check_report(issues_by_pkg: &[(String, Vec<String>)]) -> String {
    let mut out = String::new();

    for (idx, (pkgid, issues)) in issues_by_pkg.iter().enumerate() {
        if idx != 0 {
            out.push('\n');
        }
        out.push_str(pkgid);
        out.push('\n');
        for issue in issues {
            out.push_str("  ");
            out.push_str(issue);
            out.push('\n');
        }
    }

    if out.ends_with('\n') {
        out.pop();
    }

    out
}

#[cfg(test)]
mod tests {
    use crate::command::check::format_check_report;

    #[test]
    fn format_check_report_groups_issues_by_pkg() {
        let report = format_check_report(&[
            (
                "fakeblock@1.0.0:noarch".to_owned(),
                vec![
                    "Missing: /root/usr/bin/fakeblock".to_owned(),
                    "Incorrect mode: /root/etc/fakeblock.conf (expected 644; found 600)".to_owned(),
                ],
            ),
            (
                "fakeblock-songs@1.0.0:noarch".to_owned(),
                vec!["Incorrect sha256: /root/usr/share/fakeblock/songs/main-theme".to_owned()],
            ),
        ]);

        assert_eq!(
            report,
            concat!(
                "fakeblock@1.0.0:noarch\n",
                "  Missing: /root/usr/bin/fakeblock\n",
                "  Incorrect mode: /root/etc/fakeblock.conf (expected 644; found 600)\n",
                "\n",
                "fakeblock-songs@1.0.0:noarch\n",
                "  Incorrect sha256: /root/usr/share/fakeblock/songs/main-theme"
            )
        );
    }
}
