use crate::{cli::*, collection::*, error::*, file::*};

pub fn list(
    flags: CommonFlags,
    mut installed: bool,
    mut repository: bool,
    explicit: bool,
    dependency: bool,
) -> Result<String, Err> {
    let pubkeys = &PublicKeys::from_common_flags(&flags)?;
    let repository_pkgs = &RepositoryPkgs::from_root_path(&flags.root_dir, pubkeys)?;
    let installed_pkgs = &InstalledPkgs::from_root_path_ro(&flags.root_dir)?;
    let world = &World::from_root_path_ro(&flags.root_dir)?;

    let mut pkgs = Vec::new();

    if !repository && !installed && !explicit && !dependency {
        installed = true;
        repository = true;
        // explicit and dependency are sub-categories of and thus redundant with `installed`.
    }
    if repository {
        pkgs.extend(repository_pkgs.pkgids());
    }
    if installed {
        pkgs.extend(installed_pkgs.pkgids());
    }
    if !installed && explicit {
        pkgs.extend(
            installed_pkgs
                .pkgids()
                .filter(|pkgid| world.contains_match(pkgid)),
        )
    }
    if !installed && dependency {
        pkgs.extend(
            installed_pkgs
                .pkgids()
                .filter(|pkgid| !world.contains_match(pkgid)),
        )
    }

    pkgs.sort();
    pkgs.dedup();

    for pkg in pkgs {
        println!("{}", pkg.color());
    }

    // This is likely to be parsed by other programs.  Do not complicated output by printing a
    // success message.
    Ok(String::new())
}
