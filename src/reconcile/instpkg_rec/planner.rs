//! Plan construction for installed-package reconciliation.

use crate::{
    error::Err,
    metadata::{Arch, Depend, PartId, PkgId},
    reconcile::instpkg_rec::{
        CommandRequest, InstPkgPlan, InstPkgReconciler, InstallOp, NameArch, RemoveOp, ReplaceOp,
        ResolveMode, RetainOp, TargetPkgState, TargetSource, WorldChange,
    },
};
use std::collections::{HashMap, HashSet};

impl<'a> InstPkgReconciler<'a> {
    pub fn plan(&self) -> Result<InstPkgPlan, Err> {
        let mut explicit = self
            .world
            .entries()
            .iter()
            .cloned()
            .map(|partid| (partid, None))
            .collect::<HashMap<_, _>>();
        let mut extras = Vec::new();
        let mut reinstall = HashSet::new();

        match self.command {
            CommandRequest::None => {}
            CommandRequest::Install {
                pkgs,
                reinstall: do_reinstall,
            } => {
                for pkg in pkgs {
                    let resolved = self.resolve_cli_pkg(
                        pkg,
                        ResolveMode::Install {
                            reinstall: do_reinstall,
                        },
                    )?;
                    if do_reinstall {
                        reinstall.insert(resolved.source.pkgid().clone());
                        let matched = Self::matching_world_entries_for_pkgid(
                            explicit.keys().cloned().collect(),
                            resolved.source.pkgid(),
                        );
                        if matched.is_empty() {
                            extras.push(resolved.source);
                        } else {
                            for entry in matched {
                                explicit.remove(&entry);
                            }
                            explicit.insert(resolved.world_entry, Some(resolved.source));
                        }
                    } else {
                        let matched = Self::matching_world_entries_for_pkgid(
                            explicit.keys().cloned().collect(),
                            resolved.source.pkgid(),
                        );
                        for entry in matched {
                            explicit.remove(&entry);
                        }
                        explicit.insert(resolved.world_entry, Some(resolved.source));
                    }
                }
            }
            CommandRequest::Remove { pkgs } => {
                for remove in pkgs {
                    let matches = explicit
                        .keys()
                        .filter(|entry| Self::world_remove_matches(remove, entry))
                        .cloned()
                        .collect::<Vec<_>>();
                    if matches.is_empty() {
                        return Err(Err::RemovePkgNotExplicit(remove.clone()));
                    }
                    for entry in matches {
                        explicit.remove(&entry);
                    }
                }
            }
            CommandRequest::Upgrade { pkgs } => {
                if pkgs.is_empty() {
                    for pkgid in self.installed.pkgids() {
                        let source = self.resolve_repo_partid_available(
                            &PartId::new(pkgid.pkgname.clone(), None, Some(pkgid.arch)),
                            &[pkgid.arch],
                        )?;
                        if !self.assign_source_to_explicit_pkgid(&mut explicit, pkgid, source) {
                            extras.push(self.resolve_repo_partid_available(
                                &PartId::new(pkgid.pkgname.clone(), None, Some(pkgid.arch)),
                                &[pkgid.arch],
                            )?);
                        }
                    }
                } else {
                    for pkg in pkgs {
                        let resolved = self.resolve_cli_pkg(pkg, ResolveMode::Upgrade)?;
                        let current_explicit =
                            self.current_world_entries_for_pkgid(resolved.source.pkgid());
                        if current_explicit.is_empty() {
                            extras.push(resolved.source);
                        } else {
                            for entry in current_explicit {
                                explicit.remove(&entry);
                            }
                            explicit.insert(resolved.world_entry, Some(resolved.source));
                        }
                    }
                }
            }
            CommandRequest::Downgrade { pkgs } => {
                for pkg in pkgs {
                    let resolved = self.resolve_cli_pkg(pkg, ResolveMode::Downgrade)?;
                    let current_explicit =
                        self.current_world_entries_for_pkgid(resolved.source.pkgid());
                    if current_explicit.is_empty() {
                        return Err(Err::DowngradeDependencyPkg(
                            resolved.source.pkgid().to_pkgidpart(),
                        ));
                    }
                    for entry in current_explicit {
                        explicit.remove(&entry);
                    }
                    explicit.insert(resolved.world_entry, Some(resolved.source));
                }
            }
        }

        let mut targets = HashMap::<PkgId, TargetPkgState>::new();
        let mut pending = Vec::<PkgId>::new();

        for (partid, source) in explicit {
            let source = match source {
                Some(source) => source,
                None => self.resolve_partid_default(&partid)?,
            };
            Self::insert_target(&mut targets, &mut pending, source, Some(partid));
        }
        for source in extras {
            Self::insert_target(&mut targets, &mut pending, source, None);
        }

        while let Some(pkgid) = pending.pop() {
            let depends = targets
                .get(&pkgid)
                .and_then(|pkg| pkg.source.as_ref())
                .expect("target package missing source")
                .depends()?;
            for depend in depends {
                if Self::best_target_provider(&targets, &depend, &self.general.default_archs)
                    .is_some()
                {
                    continue;
                }
                let source = if let Some(instpkg) = self
                    .installed
                    .best_provider(&depend, &self.general.default_archs)
                {
                    TargetSource::Installed(instpkg.pkginfo().clone())
                } else {
                    self.resolve_repo_provider(&depend)?
                };
                Self::insert_target(&mut targets, &mut pending, source, None);
            }
        }

        self.detect_runtime_cycles(&targets)?;
        Ok(self.build_plan(targets, reinstall))
    }

    fn assign_source_to_explicit_pkgid(
        &self,
        explicit: &mut HashMap<PartId, Option<TargetSource>>,
        pkgid: &PkgId,
        source: TargetSource,
    ) -> bool {
        let Some(entry) = explicit.keys().find(|entry| entry.matches(pkgid)).cloned() else {
            return false;
        };
        explicit.insert(entry, Some(source));
        true
    }

    fn build_plan(
        &self,
        mut targets: HashMap<PkgId, TargetPkgState>,
        reinstall: HashSet<PkgId>,
    ) -> InstPkgPlan {
        let current_pkgids = self.installed.pkgids().cloned().collect::<HashSet<_>>();
        let target_pkgids = targets.keys().cloned().collect::<HashSet<_>>();

        let mut current_only = current_pkgids
            .difference(&target_pkgids)
            .cloned()
            .collect::<Vec<_>>();
        let mut target_only = target_pkgids
            .difference(&current_pkgids)
            .cloned()
            .collect::<Vec<_>>();

        current_only.sort_unstable();
        target_only.sort_unstable();

        let mut remove_by_key = current_only
            .into_iter()
            .map(|pkgid| (NameArch::from(&pkgid), pkgid))
            .collect::<HashMap<_, _>>();
        let mut install_by_key = target_only
            .into_iter()
            .map(|pkgid| (NameArch::from(&pkgid), pkgid))
            .collect::<HashMap<_, _>>();

        let mut plan = InstPkgPlan {
            world_entries: targets
                .values()
                .flat_map(|target| target.explicit_world_entries.iter().cloned())
                .collect(),
            ..Default::default()
        };

        let mut replace_keys = remove_by_key
            .keys()
            .filter(|key| install_by_key.contains_key(*key))
            .cloned()
            .collect::<Vec<_>>();
        replace_keys.sort_by(|a, b| a.pkgname.cmp(&b.pkgname).then(a.arch.cmp(&b.arch)));

        for key in replace_keys {
            let from = remove_by_key.remove(&key).expect("replace remove missing");
            let to = install_by_key
                .remove(&key)
                .expect("replace install missing");
            let source = targets
                .remove(&to)
                .and_then(|target| target.source)
                .expect("target replace source missing");
            let world_change = self.world_change_for(
                self.current_world_entries_for_pkgid(&from),
                plan.world_entries_for_pkgid(source.pkgid()),
            );
            if source.pkgid().pkgver > from.pkgver {
                plan.upgrade.push(ReplaceOp {
                    from,
                    to: source,
                    world_change,
                });
            } else if source.pkgid().pkgver < from.pkgver {
                plan.downgrade.push(ReplaceOp {
                    from,
                    to: source,
                    world_change,
                });
            }
        }

        let mut remove_pkgids = remove_by_key.into_values().collect::<Vec<_>>();
        remove_pkgids.sort_unstable();
        for pkgid in remove_pkgids {
            let world_change =
                self.world_change_for(self.current_world_entries_for_pkgid(&pkgid), HashSet::new());
            plan.remove.push(RemoveOp {
                pkgid,
                world_change,
            });
        }

        let mut install_pkgids = install_by_key.into_values().collect::<Vec<_>>();
        install_pkgids.sort_unstable();
        for pkgid in install_pkgids {
            let target = targets.remove(&pkgid).expect("install target missing");
            let world_change = self.world_change_for(
                self.current_world_entries_for_pkgid(&pkgid),
                target.explicit_world_entries.clone(),
            );
            plan.install.push(InstallOp {
                source: target.source.expect("install target source missing"),
                world_change,
            });
        }

        let mut shared = current_pkgids
            .intersection(&target_pkgids)
            .cloned()
            .collect::<Vec<_>>();
        shared.sort_unstable();
        for pkgid in shared {
            let target = targets.get(&pkgid).expect("shared target missing");
            let world_change = self.world_change_for(
                self.current_world_entries_for_pkgid(&pkgid),
                target.explicit_world_entries.clone(),
            );
            if reinstall.contains(&pkgid)
                && let Some(source) = target.source.as_ref()
                && !source.is_installed()
            {
                plan.upgrade.push(ReplaceOp {
                    from: pkgid.clone(),
                    to: targets
                        .remove(&pkgid)
                        .and_then(|target| target.source)
                        .expect("reinstall target source missing"),
                    world_change,
                });
                continue;
            }
            if let Some(world_change) = world_change {
                plan.retain.push(RetainOp {
                    pkgid,
                    world_change: Some(world_change),
                });
            }
        }

        plan.install.sort_by_key(|op| op.source.pkgid().clone());
        plan.remove.sort_by_key(|op| op.pkgid.clone());
        plan.upgrade.sort_by_key(|op| op.from.clone());
        plan.downgrade.sort_by_key(|op| op.from.clone());
        plan.retain.sort_by_key(|op| op.pkgid.clone());
        plan
    }

    fn detect_runtime_cycles(&self, targets: &HashMap<PkgId, TargetPkgState>) -> Result<(), Err> {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Visit {
            Visiting,
            Visited,
        }

        fn dfs(
            pkgid: &PkgId,
            targets: &HashMap<PkgId, TargetPkgState>,
            default_archs: &[Arch],
            visits: &mut HashMap<PkgId, Visit>,
            stack: &mut Vec<PkgId>,
        ) -> Result<(), Err> {
            if visits.get(pkgid) == Some(&Visit::Visited) {
                return Ok(());
            }
            if visits.get(pkgid) == Some(&Visit::Visiting) {
                let start = stack.iter().position(|entry| entry == pkgid).unwrap_or(0);
                let cycle = stack[start..]
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(Err::RuntimeDependencyCycle(cycle));
            }

            visits.insert(pkgid.clone(), Visit::Visiting);
            stack.push(pkgid.clone());
            let depends = targets
                .get(pkgid)
                .and_then(|target| target.source.as_ref())
                .expect("cycle target source missing")
                .depends()?;
            for depend in depends {
                if let Some(next) =
                    InstPkgReconciler::best_target_provider(targets, &depend, default_archs)
                {
                    dfs(&next, targets, default_archs, visits, stack)?;
                }
            }
            stack.pop();
            visits.insert(pkgid.clone(), Visit::Visited);
            Ok(())
        }

        let mut visits = HashMap::new();
        for pkgid in targets.keys() {
            let mut stack = Vec::new();
            dfs(
                pkgid,
                targets,
                &self.general.default_archs,
                &mut visits,
                &mut stack,
            )?;
        }
        Ok(())
    }

    fn insert_target(
        targets: &mut HashMap<PkgId, TargetPkgState>,
        pending: &mut Vec<PkgId>,
        source: TargetSource,
        explicit_world_entry: Option<PartId>,
    ) {
        let pkgid = source.pkgid().clone();
        let mut inserted = false;
        let target = targets.entry(pkgid.clone()).or_insert_with(|| {
            inserted = true;
            TargetPkgState::default()
        });
        if inserted {
            pending.push(pkgid.clone());
        }
        if let Some(explicit_world_entry) = explicit_world_entry {
            target.explicit_world_entries.insert(explicit_world_entry);
        }
        match (&target.source, source.is_installed()) {
            (None, _) => target.source = Some(source),
            (Some(existing), false) if existing.is_installed() => target.source = Some(source),
            _ => {}
        }
    }

    fn best_target_provider(
        targets: &HashMap<PkgId, TargetPkgState>,
        depend: &Depend,
        default_archs: &[Arch],
    ) -> Option<PkgId> {
        targets
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

    fn current_world_entries_for_pkgid(&self, pkgid: &PkgId) -> HashSet<PartId> {
        self.world
            .entries()
            .iter()
            .filter(|entry| entry.matches(pkgid))
            .cloned()
            .collect()
    }

    fn matching_world_entries_for_pkgid(entries: HashSet<PartId>, pkgid: &PkgId) -> Vec<PartId> {
        entries
            .into_iter()
            .filter(|entry| entry.matches(pkgid))
            .collect()
    }

    pub(super) fn world_remove_matches(remove: &PartId, entry: &PartId) -> bool {
        remove.pkgname == entry.pkgname
            && remove
                .pkgver
                .as_ref()
                .zip(entry.pkgver.as_ref())
                .is_none_or(|(a, b)| a == b)
            && remove
                .arch
                .as_ref()
                .zip(entry.arch.as_ref())
                .is_none_or(|(a, b)| a == b)
    }

    fn world_change_for(
        &self,
        current: HashSet<PartId>,
        mut target: HashSet<PartId>,
    ) -> Option<WorldChange> {
        if current == target {
            return None;
        }
        if current.is_empty() {
            let to = target.drain().next().expect("target world add missing");
            return Some(WorldChange::Add(to));
        }
        let mut from = current.into_iter().collect::<Vec<_>>();
        from.sort();
        if target.is_empty() {
            return Some(WorldChange::Remove(from));
        }
        let to = target.drain().next().expect("target world replace missing");
        Some(WorldChange::Replace { from, to })
    }
}
