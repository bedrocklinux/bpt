use crate::command::*;
use crate::error::CommandResult;
use crate::location::*;
use crate::metadata::*;
use camino::Utf8PathBuf;
use clap::Parser;
use clap::builder::{Styles, styling::AnsiColor};
use std::sync::LazyLock;

// Styling for `--help` output
const BEDROCK_CLAP_STYLE: Styles = Styles::styled()
    // Section headers, e.g. "Commands:"
    // .header(AnsiColor::Yellow.on_default().bold())
    // Error message
    .error(AnsiColor::Red.on_default().bold())
    // Literally the word "Usage:"
    // .usage(AnsiColor::Yellow.on_default().bold())
    // Literal things users should type verbatim like subcommand and flags
    .literal(AnsiColor::Green.on_default())
    // Items the user should substitute, e.g. [FLAG]
    .placeholder(AnsiColor::Yellow.on_default())
    // Suggestions for valid user input
    .valid(AnsiColor::Green.on_default())
    // Invalid user input in error messages
    .invalid(AnsiColor::Red.on_default());

#[derive(clap::Parser)]
#[clap(styles = BEDROCK_CLAP_STYLE)]
// CLAP's special-case handling of help and version flags clashes with our UX
// Disable it and implement handling like any other flag
/// Bedrock Package Tool
#[clap(
    version,
    propagate_version = true,
    disable_help_flag = true,
    disable_help_subcommand = true,
    disable_version_flag = true
)]
// Top-level `--help` output has two sections:
// 1. (sub)commands
// 2. common options
pub struct Cli {
    #[command(subcommand)]
    command: Command,
    #[clap(flatten)]
    common_flags: CommonFlags,
}

// (Sub)commands
#[derive(clap::Subcommand)]
enum Command {
    //////////////////////////////////
    // Modify installed package set //
    //////////////////////////////////
    /// Install [35mpackages[0m
    Install {
        /// May be any combination of the following:
        /// - [35mPackage identifier[0m (e.g. package name) of a [35mrepository package[0m
        /// - [32mFile path[0m to a [35mbinary package[0m ([32m*.bpt[0m)
        /// - [32mFile path[0m to a [35mbuild definition[0m ([32m*.bbuild[0m)
        /// - [38;5;33mhttp(s) URL[0m to a [35mbinary package[0m ([32m*.bpt[0m)
        /// - [38;5;33mhttp(s) URL[0m to a [35mbuild definition[0m ([32m*.bbuild[0m)
        #[clap(verbatim_doc_comment, required = true)]
        pkgs: Vec<PkgPathUrlRepo>,
        /// Reinstall already installed package(s)
        #[clap(short, long, help_heading = "Install options")]
        reinstall: bool,
    },
    /// Remove [35minstalled packages[0m
    Remove {
        /// [35mInstalled packages[0m to remove
        #[clap(verbatim_doc_comment, required = true)]
        pkgs: Vec<PartId>,
        /// Also remove modified configuration files
        #[clap(short, long, help_heading = "Remove options")]
        purge: bool,
        /// Forget package metadata without removing files from disk
        #[clap(short, long, help_heading = "Remove options")]
        forget: bool,
    },
    /// Upgrade [35minstalled packages[0m
    Upgrade {
        /// May be any combination of the following:
        /// - Empty list, indicating all [35minstalled packages[0m
        /// - [35mPackage identifier[0m (e.g. package name) of a [35mrepository package[0m
        /// - [32mFile path[0m to a [35mbinary package[0m ([32m*.bpt[0m)
        /// - [32mFile path[0m to a [35mbuild definition[0m ([32m*.bbuild[0m)
        /// - [38;5;33mhttp(s) URL[0m to a [35mbinary package[0m ([32m*.bpt[0m)
        /// - [38;5;33mhttp(s) URL[0m to a [35mbuild definition[0m ([32m*.bbuild[0m)
        #[clap(verbatim_doc_comment)]
        pkgs: Vec<PkgPathUrlRepo>,
    },
    /// Downgrade [35minstalled packages[0m
    Downgrade {
        /// May be any combination of the following:
        /// - [35mPackage identifier[0m (e.g. package name) of a [35mrepository package[0m
        /// - [32mFile path[0m to a [35mbinary package[0m ([32m*.bpt[0m)
        /// - [32mFile path[0m to a [35mbuild definition[0m ([32m*.bbuild[0m)
        /// - [38;5;33mhttp(s) URL[0m to a [35mbinary package[0m ([32m*.bpt[0m)
        /// - [38;5;33mhttp(s) URL[0m to a [35mbuild definition[0m ([32m*.bbuild[0m)
        #[clap(verbatim_doc_comment, required = true)]
        pkgs: Vec<PkgPathUrlRepo>,
    },
    /// Apply current [35mworld file[0m to the installed package set
    Apply,

    ////////////////////
    // Query database //
    ////////////////////
    /// Check [35minstalled package[0m install integrity (e.g. file checksums)
    Check {
        /// May be any combination of the following:
        /// - Empty list, indicating all [35minstalled packages[0m
        /// - [35mPackage identifier[0m (e.g. package name) of an [35minstalled package[0m
        #[clap(verbatim_doc_comment)]
        pkgs: Vec<PartId>,
        /// Treat backup file content differences as errors
        #[clap(
            short,
            long,
            help_heading = "Check options",
            conflicts_with = "ignore_backup"
        )]
        strict: bool,
        /// Ignore backup file content differences
        #[clap(short, long, help_heading = "Check options", conflicts_with = "strict")]
        ignore_backup: bool,
    },
    /// Describe [35mpackages[0m
    Info {
        /// May be any combination of the following:
        /// - [35mPackage identifier[0m (e.g. package name) of an [35minstalled package[0m
        /// - [35mPackage identifier[0m (e.g. package name) of a [35mrepository package[0m
        /// - [32mFile path[0m to a [35mbinary package[0m ([32m*.bpt[0m)
        /// - [32mFile path[0m to a [35mbuild definition[0m ([32m*.bbuild[0m)
        /// - [38;5;33mhttp(s) URL[0m to a [35mbinary package[0m ([32m*.bpt[0m)
        /// - [38;5;33mhttp(s) URL[0m to a [35mbuild definition[0m ([32m*.bbuild[0m)
        #[clap(verbatim_doc_comment, required = true)]
        pkgs: Vec<PkgPathUrlRepo>,
    },
    /// List files provided by [35mpackages[0m
    Files {
        /// May be any combination of the following:
        /// - [35mPackage identifier[0m (e.g. package name) of an [35minstalled package[0m
        /// - [35mPackage identifier[0m (e.g. package name) of a [35mrepository binary package[0m
        /// - [32mFile path[0m to a [35mbinary package[0m ([32m*.bpt[0m)
        /// - [38;5;33mhttp(s) URL[0m to a [35mbinary package[0m ([32m*.bpt[0m)
        #[clap(verbatim_doc_comment, required = true)]
        pkgs: Vec<BptPathUrlRepo>,
    },
    /// Search [35mpackages[0m
    // If no flags constrain search, searches everything.
    Search {
        /// Regular expression to match against [35mpackage[0m names or descriptions
        /// (Case insensitive unless uppercase ASCII character(s) present)
        #[clap(verbatim_doc_comment, required = true)]
        regex: String,
        /// Search [35mpackage[0m names
        #[clap(short, long, help_heading = "Search options")]
        name: bool,
        /// Search [35mpackage[0m descriptions
        #[clap(short, long, help_heading = "Search options")]
        description: bool,
        /// Search [35minstalled packages[0m
        #[clap(short, long, help_heading = "Search options")]
        installed: bool,
        /// Search [35mrepository packages[0m
        #[clap(short, long, help_heading = "Search options")]
        repository: bool,
    },
    /// List [35mpackages[0m
    // If no flags constrain list, lists everything.
    List {
        /// List [35minstalled packages[0m
        #[clap(short, long, help_heading = "List options")]
        installed: bool,
        /// List [35mrepository packages[0m
        #[clap(short, long, help_heading = "List options")]
        repository: bool,
        /// List explicitly [35minstalled packages[0m
        #[clap(short = 'x', long, help_heading = "List options")]
        explicit: bool,
        /// List [35mpackages[0m installed as dependencies
        #[clap(short = 'd', long, help_heading = "List options")]
        dependency: bool,
    },
    /// List [35mpackages[0m that provide files
    // If no flags constrain list, lists everything.
    Provides {
        /// Regex to match against [35mpackage[0m file paths
        /// Case insensitive unless uppercase ASCII character(s) present
        #[clap(required = true)]
        regex: String,
        /// Search [35minstalled packages[0m
        #[clap(short, long, help_heading = "Provides options")]
        installed: bool,
        /// Search [35mrepository packages[0m
        #[clap(short, long, help_heading = "Provides options")]
        repository: bool,
    },

    /////////////////////////
    // Repository requests //
    /////////////////////////
    /// Sync [35mrepository[0m information
    Sync {
        /// May be any combination of the following:
        /// - Empty list, indicating all [35mconfigured indexes[0m
        /// - [38;5;33mhttp(s) URL[0m to a [35mpackage index[0m ([32m*.pkgidx[0m)
        /// - [38;5;33mhttp(s) URL[0m to a [35mfile index[0m ([32m*.fileidx[0m)
        /// - [32mFile path[0m to a [35mpackage index[0m ([32m*.pkgidx[0m)
        /// - [32mFile path[0m to a [35mfile index[0m ([32m*.fileidx[0m)
        #[clap(verbatim_doc_comment)]
        indexes: Vec<IdxPathUrl>,
        /// Refresh indexes even if they were checked recently
        #[clap(short, long, help_heading = "Sync options")]
        force: bool,
    },
    /// Fetch [35mpackages[0m from repositories
    Fetch {
        /// [35mPackage identifier[0m (e.g. package name) of a [35mrepository package[0m
        #[clap(required = true)]
        pkgs: Vec<PartId>,
    },
    /// Remove cached [35mpackages[0m and/or [35msource[0m files
    Clean {
        /// Remove cached [35mpackages[0m
        #[clap(short, long, help_heading = "Clean options")]
        packages: bool,
        /// Remove cached [35msource[0m files
        #[clap(short = 's', long, help_heading = "Clean options")]
        source: bool,
    },

    //////////////////////
    // Package building //
    //////////////////////
    /// Build [35mbinary packages[0m ([32m*.bpt[0m) from [35mbuild definitions[0m ([32m*.bbuild[0m)
    Build {
        /// May be any combination of the following:
        /// - [35mPackage identifier[0m (e.g. package name) buildable from [35mrepository build definitions[0m
        /// - [38;5;33mhttp(s) URL[0m to a [35mbuild definition[0m ([32m*.bbuild[0m)
        /// - [32mFile path[0m to a [35mbuild definition[0m ([32m*.bbuild[0m)
        #[clap(verbatim_doc_comment, required = true)]
        bbuilds: Vec<BbuildPathUrlRepo>,
        /// Target [35marchitecture[0m
        #[clap(short, long, value_enum, default_value = Arch::host().as_str(), help_heading = "Build options")]
        arch: Arch,
    },
    /// Generate local [35mrepository[0m [32m*.bpt[0m, [32m*.pkgidx[0m, and [32m*.fileidx[0m files
    MakeRepo,

    ////////////////
    // Signatures //
    ////////////////
    /// Verify [35msignatures[0m
    Verify {
        /// [32mFile paths[0m to signed files to verify
        #[clap(required = true)]
        paths: Vec<Utf8PathBuf>,
    },
    /// Sign files, stripping preexisting [35msignatures[0m if necessary.
    Sign {
        /// Only (re)sign files which do not currently pass `bpt verify`
        #[clap(short, long)]
        needed: bool,
        /// [32mFile paths[0m to files to (re)sign
        #[clap(required = true)]
        paths: Vec<Utf8PathBuf>,
    },
}

const COMMON: &str = "Common options";

// False positive warning on Clap's manual help
#[allow(clippy::manual_non_exhaustive)]
#[derive(Parser)]
pub struct CommonFlags {
    /// Display this help information
    #[clap(short = 'h', long, global = true, help_heading = COMMON, action = clap::ArgAction::HelpLong)]
    help: (),

    /// Display version information
    #[clap(short = 'v', long, global = true, help_heading = COMMON, action = clap::ArgAction::Version)]
    version: (),
    /// Assume "yes" as answer to all prompts and run non-interactively.
    #[clap(short = 'y', long, global = true, help_heading = COMMON)]
    pub yes: bool,

    /// Show steps that would be taken without taking them.
    #[clap(short = 'D', long, global = true, help_heading = COMMON)]
    pub dry_run: bool,

    /// Print network utility stderr
    #[clap(short = 'N', long, global = true, help_heading = COMMON)]
    pub netutil_stderr: bool,

    /// Skip verifying [35msignatures[0m
    #[clap(short = 'V', long, global = true, help_heading = COMMON)]
    pub skip_verify: bool,

    /// Skip signing results
    #[clap(short = 'S', long, global = true, help_heading = COMMON)]
    pub skip_sign: bool,

    /// Minisign [35mprivate key[0m (aka [35msecret key[0m)
    #[clap(short = 'P', long, global = true, default_value = default_priv_key_path(), help_heading = COMMON)]
    pub priv_key: Utf8PathBuf,

    /// File containing minisign private key passphrase for non-interactive use
    #[clap(long, global = true, help_heading = COMMON)]
    pub priv_key_passphrase_file: Option<Utf8PathBuf>,

    /// Output directory for fetched or built files
    #[clap(short = 'O', long, global = true, default_value = default_out_dir_path(), help_heading = COMMON)]
    pub out_dir: Utf8PathBuf,

    /// Manage file system at root
    #[clap(short = 'R', long, global = true, default_value = "/", help_heading = COMMON)]
    pub root_dir: RootDir,
}

fn default_priv_key_path() -> &'static str {
    // Clap 4.0 only accepts defaults as reference types (&str or &OsStr).  In order to support this,
    // (ab)use a dynamically populated static value.
    static DEFAULT_PRIV_KEY: LazyLock<Utf8PathBuf> = LazyLock::new(|| {
        dirs::home_dir()
            .and_then(|mut p| {
                p.push(".minisign/minisign.key");
                Utf8PathBuf::try_from(p).ok()
            })
            .expect("Unable to get home directory")
    });

    DEFAULT_PRIV_KEY.as_str()
}

fn default_out_dir_path() -> &'static str {
    // Clap 4.0 only accepts defaults as reference types (&str or &OsStr).  In order to support this,
    // (ab)use a dynamically populated static value.
    static DEFAULT_OUT_DIR: LazyLock<Utf8PathBuf> = LazyLock::new(|| {
        std::env::current_dir()
            .and_then(|cwd| Utf8PathBuf::try_from(cwd).map_err(|e| e.into_io_error()))
            .expect("Unable to get current working directory")
    });

    DEFAULT_OUT_DIR.as_str()
}

impl Cli {
    pub fn run(self) -> CommandResult {
        let Self {
            command,
            common_flags,
        } = self;

        match command {
            Command::Install { pkgs, reinstall } => install(common_flags, pkgs, reinstall),
            Command::Remove {
                pkgs,
                purge,
                forget,
            } => remove(common_flags, pkgs, purge, forget),
            Command::Upgrade { pkgs } => upgrade(common_flags, pkgs),
            Command::Downgrade { pkgs } => downgrade(common_flags, pkgs),
            Command::Apply => apply(common_flags),
            Command::Check {
                pkgs,
                strict,
                ignore_backup,
            } => check(common_flags, pkgs, strict, ignore_backup),
            Command::Info { pkgs } => info(common_flags, pkgs),
            Command::Files { pkgs } => files(common_flags, pkgs),
            Command::Search {
                regex,
                name,
                description,
                installed,
                repository,
            } => search(
                common_flags,
                regex,
                name,
                description,
                installed,
                repository,
            ),
            Command::List {
                installed,
                repository,
                explicit,
                dependency,
            } => list(common_flags, installed, repository, explicit, dependency),
            Command::Provides {
                regex,
                installed,
                repository,
            } => provides(common_flags, regex, installed, repository),
            Command::Sync { indexes, force } => sync(common_flags, indexes, force),
            Command::Fetch { pkgs } => fetch(common_flags, pkgs),
            Command::Clean { packages, source } => clean(common_flags, packages, source),
            Command::Build { bbuilds, arch } => build(common_flags, bbuilds, arch),
            Command::MakeRepo => make_repo(common_flags),
            Command::Verify { paths } => verify(common_flags, paths),
            Command::Sign { needed, paths } => sign(common_flags, needed, paths),
        }
    }
}
