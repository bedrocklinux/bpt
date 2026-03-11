//! Generic system to reconcile a current state with a target state.
//!
//! Used for things like:
//! - Managing a local repo, comparing the current and target states of packages and indexes
//! - Managing installed package state, comparing current to target list of installed packages

use crate::error::Err;
use std::{
    collections::{HashMap, HashSet},
    fmt::Formatter,
    hash::Hash,
};

pub trait Reconciler<'a> {
    /// Data used to correlate current and target states
    type Key: Hash + Eq + Ord;
    /// Information about the current state
    type Current;
    /// Information about the target state
    type Target;
    /// Information needed to apply a reconciliation plan
    type ApplyArgs;

    /// Compare current and target states to determine needed changes
    fn cmp(key: &Self::Key, current: &Self::Current, target: &Self::Target) -> std::cmp::Ordering;

    // Gather current state
    fn current(&self) -> &HashMap<Self::Key, Self::Current>;
    // Gather target state
    fn target(&self) -> &HashMap<Self::Key, Self::Target>;

    // Reconcile state
    fn create(key: &Self::Key, target: &Self::Target, args: &Self::ApplyArgs) -> Result<(), Err>;
    fn remove(key: &Self::Key, current: &Self::Current, args: &Self::ApplyArgs) -> Result<(), Err>;
    fn upgrade(
        key: &Self::Key,
        current: &Self::Current,
        target: &Self::Target,
        args: &Self::ApplyArgs,
    ) -> Result<(), Err>;
    fn downgrade(
        _key: &Self::Key,
        _current: &Self::Current,
        _target: &Self::Target,
        _args: &Self::ApplyArgs,
    ) -> Result<(), Err> {
        // Some reconcilers can never downgrade.  If so, they can skip implementing this.
        unreachable!("Reconciler which downgrades did not implement downgrades")
    }

    // Describe planned changes, e.g. to prompt whether to continue
    //
    // Some reconciler never print anything here.  If so, they can skip implementing these.
    fn create_desc(
        _key: &Self::Key,
        _target: &Self::Target,
        _f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        unreachable!("Reconciler never prints create descriptions")
    }
    fn remove_desc(
        _key: &Self::Key,
        _current: &Self::Current,
        _f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        unreachable!("Reconciler never prints remove descriptions")
    }
    fn upgrade_desc(
        _key: &Self::Key,
        _current: &Self::Current,
        _target: &Self::Target,
        _f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        unreachable!("Reconciler never prints upgrade descriptions")
    }
    fn downgrade_desc(
        _key: &Self::Key,
        _current: &Self::Current,
        _target: &Self::Target,
        _f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        unreachable!("Reconciler which downgrades did not implement downgrades")
    }

    /// Apply a reconciliation plan.
    ///
    /// By default this applies changes in section order:
    /// remove -> create -> upgrade -> downgrade.
    /// Reconciler implementations may override this to enforce custom ordering.
    fn apply_plan(plan: &ReconcilePlan<'a, Self>, args: &Self::ApplyArgs) -> Result<(), Err>
    where
        Self: Sized,
    {
        for (key, current) in &plan.remove {
            Self::remove(key, current, args)?;
        }
        for (key, target) in &plan.create {
            Self::create(key, target, args)?;
        }
        for (key, current, target) in &plan.upgrade {
            Self::upgrade(key, current, target, args)?;
        }
        for (key, current, target) in &plan.downgrade {
            Self::downgrade(key, current, target, args)?;
        }
        Ok(())
    }

    fn plan(&'a self) -> ReconcilePlan<'a, Self>
    where
        Self: std::marker::Sized,
    {
        let current = self.current();
        let target = self.target();
        let current_keys = current.keys().collect::<HashSet<_>>();
        let target_keys = target.keys().collect::<HashSet<_>>();

        let mut create = target_keys
            .difference(&current_keys)
            .map(|&key| (key, target.get(key).unwrap()))
            .collect::<Vec<_>>();
        create.sort_by_key(|(a, _)| *a);

        let mut remove = current_keys
            .difference(&target_keys)
            .map(|&key| (key, current.get(key).unwrap()))
            .collect::<Vec<_>>();
        remove.sort_by_key(|(a, _)| *a);

        let mut upgrade = current_keys
            .intersection(&target_keys)
            .filter(|key| {
                let current = current.get(key).unwrap();
                let target = target.get(key).unwrap();
                Self::cmp(key, current, target) == std::cmp::Ordering::Less
            })
            .map(|&key| (key, current.get(key).unwrap(), target.get(key).unwrap()))
            .collect::<Vec<_>>();
        upgrade.sort_by_key(|(a, _, _)| *a);

        let mut downgrade = current_keys
            .intersection(&target_keys)
            .filter(|key| {
                let current = current.get(key).unwrap();
                let target = target.get(key).unwrap();
                Self::cmp(key, current, target) == std::cmp::Ordering::Greater
            })
            .map(|&key| (key, current.get(key).unwrap(), target.get(key).unwrap()))
            .collect::<Vec<_>>();
        downgrade.sort_by_key(|(a, _, _)| *a);

        ReconcilePlan {
            create,
            remove,
            upgrade,
            downgrade,
        }
    }
}

pub struct ReconcilePlan<'a, T: Reconciler<'a>> {
    pub create: Vec<(&'a T::Key, &'a T::Target)>,
    pub remove: Vec<(&'a T::Key, &'a T::Current)>,
    pub upgrade: Vec<(&'a T::Key, &'a T::Current, &'a T::Target)>,
    pub downgrade: Vec<(&'a T::Key, &'a T::Current, &'a T::Target)>,
}

impl<'a, T: Reconciler<'a>> std::fmt::Display for ReconcilePlan<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (key, target) in &self.remove {
            T::remove_desc(key, target, f)?;
        }
        for (key, target) in &self.create {
            T::create_desc(key, target, f)?;
        }
        for (key, current, target) in &self.upgrade {
            T::upgrade_desc(key, current, target, f)?;
        }
        for (key, current, target) in &self.downgrade {
            T::downgrade_desc(key, current, target, f)?;
        }
        Ok(())
    }
}

impl<'a, T: Reconciler<'a>> ReconcilePlan<'a, T> {
    pub fn apply(&self, args: &T::ApplyArgs) -> Result<(), Err> {
        T::apply_plan(self, args)
    }

    pub fn is_empty(&self) -> bool {
        self.create.is_empty()
            && self.remove.is_empty()
            && self.upgrade.is_empty()
            && self.downgrade.is_empty()
    }
}
