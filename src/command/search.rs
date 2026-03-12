use crate::{cli::*, collection::*, color::*, error::*, file::*};

pub fn search(
    flags: CommonFlags,
    regex: String,
    mut name: bool,
    mut description: bool,
    mut installed: bool,
    mut repository: bool,
) -> Result<String, Err> {
    let pubkeys = &PublicKeys::from_common_flags(&flags)?;
    let installed_pkgs = &InstalledPkgs::from_root_path_ro(&flags.root_dir)?;
    let repository_pkgs = &RepositoryPkgs::from_root_path(&flags.root_dir, pubkeys)?;

    let case_insensitive = !regex.chars().any(|c| c.is_ascii_uppercase());
    let regex = regex_lite::RegexBuilder::new(&regex)
        .case_insensitive(case_insensitive)
        .build()
        .map_err(|e| Err::InvalidRegex(regex, e))?;

    let mut pkgs = Vec::new();
    // No filter flags indicates all are requested
    if !name && !description {
        name = true;
        description = true;
    }
    if !installed && !repository {
        installed = true;
        repository = true;
    }
    if repository {
        pkgs.extend(repository_pkgs.pkgid_pkgdesc())
    }
    if installed {
        pkgs.extend(installed_pkgs.pkgid_pkgdesc());
    }

    pkgs.sort_by(|(pkgid1, _), (pkgid2, _)| pkgid1.cmp(pkgid2));
    pkgs.dedup();

    let width = pkgs
        .iter()
        .map(|(pkgid, _)| {
            // If matching against name, work against non-colorized version both to make the match work
            // and to allow us to clearly colorized matched component.
            if name {
                pkgid.to_string().len() + 1
            } else {
                pkgid.color().to_string().len() + 1
            }
        })
        .max()
        .unwrap_or(0);

    for (pkgid, pkgdesc) in pkgs {
        let pkgname = pkgid.pkgname.as_str();
        let pkgdesc = pkgdesc.as_str();
        let name_match = name && regex.is_match(pkgname);
        let desc_match = description && regex.is_match(pkgdesc);
        if !name_match && !desc_match {
            continue;
        }

        // If matching against name, work against non-colorized version both to make the match work
        // and to allow us to clearly colorized matched component.
        let pkgid = if name {
            pkgid.to_string()
        } else {
            pkgid.color().to_string()
        };

        if name_match {
            let mut start = 0;
            for mat in regex.find_iter(pkgname) {
                print!("{}", &pkgid[start..mat.start()]);
                print!("{}{}{}", Color::Match, mat.as_str(), Color::Default);
                start = mat.end();
            }
            print!("{}", &pkgid[start..]);
        } else {
            print!("{pkgid}");
        };

        print!("{:<pad$}", "", pad = width - pkgid.len());

        if desc_match {
            let mut start = 0;
            for mat in regex.find_iter(pkgdesc) {
                print!("{}", &pkgdesc[start..mat.start()]);
                print!("{}{}{}", Color::Match, mat.as_str(), Color::Default);
                start = mat.end();
            }
            print!("{}", &pkgdesc[start..]);
        } else {
            print!("{pkgdesc}");
        }

        println!();
    }

    // This is likely to be parsed by other programs.  Do not complicated output by printing a
    // success message.
    Ok(String::new())
}
