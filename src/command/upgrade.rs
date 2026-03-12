use crate::{
    cli::*, collection::*, color::Color, constant::*, error::*, file::*, io::*, location::*,
    reconcile::*,
};
use camino::Utf8PathBuf;
use std::{cell::RefCell, fs::create_dir_all};

pub fn upgrade(flags: CommonFlags, pkgs: Vec<PkgPathUrlRepo>) -> Result<String, Err> {
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
            command: CommandRequest::Upgrade { pkgs: &pkgs },
        }
        .plan()?
    };

    if plan.is_empty() {
        return Ok("No changes needed".to_string());
    }

    if flags.dry_run {
        println!("{}Would have:{}\n{plan}", Color::Warn, Color::Default);
        println!();
        return Ok(format!("Dry ran {}", plan.summary().to_lowercase()));
    }

    println!("Continuing will:\n{plan}");
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
    let build_support = if plan.needs_builds() {
        let build_credentials = bpt_conf.build_credentials()?;
        Some(UpgradeBuildSupport {
            adjusted_root_dir: flags.root_dir.adjust_bedrock_prefix()?,
            // `upgrade` only builds ephemeral packages that are consumed immediately by the
            // current process. It does not emit distributable output, so prompting for a signing
            // key here is unnecessary.
            privkey: PrivKey::SkipSign,
            build_credentials,
            make_conf: MakeConf::from_root_path(&flags.root_dir)?,
            make_common: MakeCommon::from_root_path(&flags.root_dir)?,
            tmpdir: RefCell::new(TmpDir::new(bpt_conf)?),
            src_cache: RefCell::new(Cache::from_root_path(
                &flags.root_dir,
                SRC_CACHE,
                "source cache",
            )?),
        })
    } else {
        None
    };
    let buildargs = build_support
        .as_ref()
        .map(|support| support.as_build_args(&flags, netutil, installed, available_bpts));
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
        buildargs: buildargs.as_ref(),
    })?;
    print_bptnew(&bptnew);

    println!();
    Ok("Updated installed package set".to_string())
}

struct UpgradeBuildSupport {
    adjusted_root_dir: Utf8PathBuf,
    privkey: PrivKey,
    build_credentials: Option<ProcessCredentials>,
    make_conf: MakeConf,
    make_common: MakeCommon,
    tmpdir: RefCell<TmpDir>,
    src_cache: RefCell<Cache>,
}

impl UpgradeBuildSupport {
    fn as_build_args<'a>(
        &'a self,
        flags: &'a CommonFlags,
        netutil: &'a RefCell<NetUtil<'a>>,
        installed_pkgs: &'a InstalledPkgs,
        available_bpts: &'a RefCell<AvailableBpts>,
    ) -> BuildArgs<'a> {
        BuildArgs {
            privkey: &self.privkey,
            build_credentials: self.build_credentials.as_ref(),
            make_conf: &self.make_conf,
            make_common: &self.make_common,
            root_dir: &self.adjusted_root_dir,
            out_dir: &flags.out_dir,
            tmpdir: &self.tmpdir,
            netutil,
            src_cache: &self.src_cache,
            installed_pkgs,
            available_bpts,
        }
    }
}

fn print_bptnew(paths: &[Utf8PathBuf]) {
    if paths.is_empty() {
        return;
    }

    println!();
    for path in paths {
        println!("{}Created{} {}.bptnew", Color::Warn, Color::Default, path);
    }
}
