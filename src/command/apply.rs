use crate::{
    cli::*, collection::*, color::Color, constant::*, error::*, file::*, io::*, reconcile::*,
};
use std::cell::RefCell;
use std::fs::create_dir_all;

pub fn apply(flags: CommonFlags) -> Result<String, Err> {
    let bpt_conf = &BptConf::from_root_path(&flags.root_dir)?;
    let plan = {
        let pubkeys = &PublicKeys::from_common_flags(&flags)?;
        let repository = &RepositoryPkgs::from_root_path(&flags.root_dir, pubkeys)?;
        let installed = &InstalledPkgs::from_root_path_ro(&flags.root_dir)?;
        let world = &World::from_root_path_ro(&flags.root_dir)?;
        let netutil = &RefCell::new(NetUtil::new(bpt_conf, flags.netutil_stderr));
        let query_credentials = &QueryCredentials::new(bpt_conf);
        let pkgcache = &RefCell::new(Cache::from_root_path(
            &flags.root_dir,
            PKG_CACHE,
            "package cache",
        )?);
        InstPkgReconciler {
            world,
            installed,
            repository,
            pubkeys,
            netutil,
            pkgcache,
            general: &bpt_conf.general,
            query_credentials,
            command: CommandRequest::None,
        }
        .plan()?
    };

    if plan.is_empty() {
        return Ok("No changes needed".to_string());
    }

    if flags.dry_run {
        println!("{}Would have:{}\n{plan}", Color::Warn, Color::Default);
        return Ok(format!("Dry ran {}", plan.summary().to_lowercase()));
    }

    print!("Continuing will:\n{plan}");
    if !flags.yes && !confirm()? {
        return Err(Err::ConfirmDenied);
    }

    let pubkeys = &PublicKeys::from_common_flags(&flags)?;
    let repository = &RepositoryPkgs::from_root_path(&flags.root_dir, pubkeys)?;
    let installed = &InstalledPkgs::from_root_path_rw(&flags.root_dir)?;
    let mut world = World::from_root_path_rw(&flags.root_dir)?;
    let netutil = &RefCell::new(NetUtil::new(bpt_conf, flags.netutil_stderr));
    let pkgcache = &RefCell::new(Cache::from_root_path(
        &flags.root_dir,
        PKG_CACHE,
        "package cache",
    )?);
    let instpkg_dir = flags.root_dir.as_path().join(INSTPKG_DIR_PATH);
    create_dir_all(&instpkg_dir).map_err(|e| Err::CreateDir(instpkg_dir.to_string(), e))?;
    let available_bpts = &RefCell::new(AvailableBpts::new());
    let bptnew = plan.apply(InstPkgApplyArgs {
        root: &flags.root_dir,
        installed,
        world: &mut world,
        instpkg_dir: &instpkg_dir,
        purge: false,
        forget: false,
        repository,
        pubkeys,
        netutil,
        pkgcache,
        available_bpts,
        buildargs: None,
    })?;

    if !bptnew.is_empty() {
        println!();
        for path in bptnew {
            println!("{}Created{} {}.bptnew", Color::Warn, Color::Default, path);
        }
    }

    Ok("Updated installed package set".to_string())
}
