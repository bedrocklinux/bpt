//! (Partial) Package ID corresponding to an repository [Bbuild] file in the repositories.

use crate::metadata::*;
use std::ops::Deref;

#[derive(Clone)]
pub struct BbuildRepo(PartId);

impl BbuildRepo {
    pub fn from_partid(id: PartId) -> Self {
        Self(id)
    }
}

impl Deref for BbuildRepo {
    type Target = PartId;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
