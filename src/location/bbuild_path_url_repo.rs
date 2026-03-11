//! Location a [Bbuild] file.  May be any of:
//!
//! - File path to a [Bbuild] file
//! - http(s) URL to a [Bbuild] file
//! - [PartId] to an repository [Bbuild] file

use crate::{error::*, location::*, metadata::*};
use std::str::FromStr;

#[derive(Clone)]
pub enum BbuildPathUrlRepo {
    Path(BbuildPath),
    Url(BbuildUrl),
    Repo(BbuildRepo),
}

impl FromStr for BbuildPathUrlRepo {
    type Err = Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("http://") || s.starts_with("https://") {
            BbuildUrl::from_str(s).map(BbuildPathUrlRepo::Url)
        } else if s.contains('/') || s.ends_with(".bbuild") {
            BbuildPath::from_str(s).map(BbuildPathUrlRepo::Path)
        } else if let Ok(id) = PartId::from_str(s) {
            let buildrepo = BbuildRepo::from_partid(id);
            Ok(BbuildPathUrlRepo::Repo(buildrepo))
        } else {
            Err(Err::InvalidBbuildPathUrlRepo(s.to_owned()))
        }
    }
}
