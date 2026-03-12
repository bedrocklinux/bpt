use crate::{cli::*, collection::*, color::*, error::*, file::*, metadata::*};
use camino::Utf8PathBuf;
use std::collections::BTreeMap;

pub fn provides(
    flags: CommonFlags,
    regex: String,
    mut installed: bool,
    mut repository: bool,
) -> Result<String, Err> {
    let pubkeys = &PublicKeys::from_common_flags(&flags)?;
    let pkg_files = PkgFiles::from_root_path(&flags.root_dir, pubkeys)?;
    let installed_pkgs = InstalledPkgs::from_root_path_ro(&flags.root_dir)?;

    let case_insensitive = !regex.chars().any(|c| c.is_ascii_uppercase());
    let regex = regex_lite::RegexBuilder::new(&regex)
        .case_insensitive(case_insensitive)
        .build()
        .map_err(|e| Err::InvalidRegex(regex, e))?;

    let mut pkgs = BTreeMap::<PkgId, Vec<Utf8PathBuf>>::new();
    // No filter flags indicates all are requested
    if !installed && !repository {
        installed = true;
        repository = true;
    }
    if installed {
        for (pkgid, paths) in installed_pkgs.pkgid_paths() {
            pkgs.insert(
                pkgid.clone(),
                paths.into_iter().map(|path| path.to_path_buf()).collect(),
            );
        }
    }
    if repository {
        for (pkgid, paths) in pkg_files.pkgid_paths() {
            pkgs.entry(pkgid.clone())
                .or_insert_with(|| paths.into_iter().map(|path| path.to_path_buf()).collect());
        }
    }

    let width = pkgs
        .iter()
        .filter(|(_, paths)| paths.iter().any(|path| regex.is_match(path.as_str())))
        .map(|(pkgid, _)| pkgid.color().to_string().len() + 1)
        .max()
        .unwrap_or(0);

    for (pkgid, paths) in pkgs {
        if !paths.iter().any(|path| regex.is_match(path.as_str())) {
            continue;
        }

        // Pre-formatting pkgid is needed for format!() padding to work.
        let pkgid = pkgid.color().to_string();

        for path in paths {
            let path = path.as_str();
            if !regex.is_match(path) {
                continue;
            }

            print!("{pkgid:width$}");
            let mut start = 0;
            for mat in regex.find_iter(path) {
                print!("{}", &path[start..mat.start()]);
                print!("{}{}{}", Color::Match, mat.as_str(), Color::Default);
                start = mat.end();
            }
            println!("{}", &path[start..]);
        }
    }

    // This is likely to be parsed by other programs.  Do not complicated output by printing a
    // success message.
    Ok(String::new())
}
