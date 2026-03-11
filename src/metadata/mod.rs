//! Metadata fields of various bpt file types.

// Auxiliary and miscellaneous modules
mod instpkgkey;
mod pkgkey;
mod pkgsrc;
mod timestamp;
pub use instpkgkey::*;
pub use pkgkey::*;
pub use pkgsrc::*;
pub use timestamp::*;

// Package metadata fields
mod arch;
mod backup;
mod depend;
mod depends;
mod filepath;
mod homepage;
mod license;
mod makearchs;
mod makebin;
mod makebins;
mod makedepends;
mod pkgdesc;
mod pkgname;
mod pkgver;
mod repopath;
mod versionreq;
pub use arch::*;
pub use backup::*;
pub use depend::*;
pub use depends::*;
pub use filepath::*;
pub use homepage::*;
pub use license::*;
pub use makearchs::*;
pub use makebin::*;
pub use makebins::*;
pub use makedepends::*;
pub use pkgdesc::*;
pub use pkgname::*;
pub use pkgver::*;
pub use repopath::*;
pub use versionreq::*;

// Instpkg metadata fields
mod dir;
mod filename;
mod gid;
mod mode;
mod regfile;
mod subdir;
mod symlink;
mod uid;
pub use dir::*;
pub use filename::*;
pub use gid::*;
pub use mode::*;
pub use regfile::*;
pub use subdir::*;
pub use symlink::*;
pub use uid::*;

// Combinations of multiple package metadata fields
mod instfile;
mod partid;
mod pkgid;
mod pkginfo;
pub use instfile::*;
pub use partid::*;
pub use pkgid::*;
pub use pkginfo::*;
