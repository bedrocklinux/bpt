//! Resource locations
//!
//! Abstractions over the various places a given resource could be located.
//!
//! These attempt to reduce to the corresponding enum variant as eagerly as they can in order to
//! error early, but stop when requiring external resources which may not yet be available such as
//! public keys or networking utilities.

mod bbuild_path;
mod bbuild_path_url_repo;
mod bbuild_repo;
mod bbuild_url;
mod bpt_path;
mod bpt_path_url_repo;
mod bpt_repo;
mod bpt_url;
mod idx;
mod idx_path;
mod idx_path_url;
mod idx_url;
mod pkg;
mod pkg_path;
mod pkg_path_url;
mod pkg_path_url_repo;
mod pkg_url;
mod root_dir;
mod url;
pub use bbuild_path::*;
pub use bbuild_path_url_repo::*;
pub use bbuild_repo::*;
pub use bbuild_url::*;
pub use bpt_path::*;
pub use bpt_path_url_repo::*;
pub use bpt_repo::*;
pub use bpt_url::*;
pub use idx::*;
pub use idx_path::*;
pub use idx_path_url::*;
pub use idx_url::*;
pub use pkg::*;
pub use pkg_path::*;
pub use pkg_path_url::*;
pub use pkg_path_url_repo::*;
pub use pkg_url::*;
pub use root_dir::*;
pub use url::*;
