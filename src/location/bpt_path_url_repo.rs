//! Location of a [Bpt] file. May be any of:
//!
//! - File path to a [Bpt] file
//! - http(s) URL to a [Bpt] file
//! - [PartId] to a repository [Bpt] file

use crate::{error::*, location::*, metadata::*};
use std::str::FromStr;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BptPathUrlRepo {
    Path(BptPath),
    Url(BptUrl),
    Repo(BptRepo),
}

impl std::fmt::Display for BptPathUrlRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BptPathUrlRepo::Path(s) => write!(f, "{s}"),
            BptPathUrlRepo::Url(s) => write!(f, "{s}"),
            BptPathUrlRepo::Repo(s) => write!(f, "{s}"),
        }
    }
}

impl FromStr for BptPathUrlRepo {
    type Err = Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("http://") || s.starts_with("https://") {
            BptUrl::from_str(s).map(BptPathUrlRepo::Url)
        } else if s.contains('/') || s.ends_with(".bpt") || s.ends_with(".bbuild") {
            BptPath::from_str(s).map(BptPathUrlRepo::Path)
        } else if let Ok(id) = PartId::from_str(s) {
            Ok(BptPathUrlRepo::Repo(BptRepo::from_partid(id)))
        } else {
            Err(Err::InvalidBptPathUrlRepo(s.to_owned()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bbuild_suffix_is_parsed_as_path_for_later_type_validation() {
        let parsed = BptPathUrlRepo::from_str("fakeblock.bbuild").unwrap();
        assert!(matches!(parsed, BptPathUrlRepo::Path(_)));
    }

    #[test]
    fn bare_identifier_is_parsed_as_repo_lookup() {
        let parsed = BptPathUrlRepo::from_str("fakeblock").unwrap();
        assert!(matches!(parsed, BptPathUrlRepo::Repo(_)));
    }
}
