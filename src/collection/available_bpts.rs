use crate::{file::Bpt, metadata::*};
use std::collections::HashMap;

/// Immediately available binary packages.
pub struct AvailableBpts {
    bpts: HashMap<PkgId, Bpt>,
}

impl AvailableBpts {
    pub fn new() -> Self {
        Self {
            bpts: HashMap::new(),
        }
    }

    pub fn add(&mut self, bpt: Bpt) {
        self.bpts.insert(bpt.pkgid().clone(), bpt);
    }

    pub fn remove(&mut self, pkgid: &PkgId) -> Option<Bpt> {
        self.bpts.remove(pkgid)
    }

    pub fn get(&self, pkgid: &PkgId) -> Option<&Bpt> {
        self.bpts.get(pkgid)
    }

    pub fn get_mut(&mut self, pkgid: &PkgId) -> Option<&mut Bpt> {
        self.bpts.get_mut(pkgid)
    }

    // Find the package id that best provides a given dependency.
    pub fn best_provider_pkgid(&self, depend: &Depend, default_archs: &[Arch]) -> Option<PkgId> {
        self.bpts
            .keys()
            .filter(|pkgid| depend.provided_by(pkgid))
            .fold(None, |best: Option<&PkgId>, cur| match best {
                None => Some(cur),
                Some(best) => match best.better_match_than(cur, default_archs) {
                    std::cmp::Ordering::Greater | std::cmp::Ordering::Equal => Some(best),
                    std::cmp::Ordering::Less => Some(cur),
                },
            })
            .cloned()
    }
}
