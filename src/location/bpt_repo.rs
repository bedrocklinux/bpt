//! (Partial) Package ID corresponding to a repository [Bpt] file in the repositories.

use crate::metadata::*;
use std::ops::Deref;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BptRepo(PartId);

impl BptRepo {
    pub fn from_partid(id: PartId) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for BptRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for BptRepo {
    type Target = PartId;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
