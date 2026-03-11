use crate::{error::Err, file::Bbuild, marshalling::*, metadata::*};
use std::collections::{BTreeSet, HashMap, HashSet};

/// A package to build, plus the context needed to build it.
pub struct BuildTarget<'a> {
    pub pkgid: PkgId,
    pub bbuild: &'a Bbuild,
    pub arch: Arch,
}

/// Sort build targets into an order where build-time dependencies are built first.
///
/// Dependencies that are not part of the target set are ignored here; callers are expected to
/// satisfy those from installed packages and/or prebuilt available packages.
pub fn sort_build_targets<'a>(targets: Vec<BuildTarget<'a>>) -> Result<Vec<BuildTarget<'a>>, Err> {
    if targets.is_empty() {
        return Ok(Vec::new());
    }

    let mut pkgid_to_idx = HashMap::new();
    for (idx, target) in targets.iter().enumerate() {
        pkgid_to_idx.insert(target.pkgid.clone(), idx);
    }

    let mut indegree = vec![0usize; targets.len()];
    let mut edges = vec![HashSet::<usize>::new(); targets.len()];

    for (dep_idx, target) in targets.iter().enumerate() {
        let makedepends = target
            .bbuild
            .pkginfo()
            .makedepends
            .populate_depends_arch_if_missing(target.arch);
        let candidate_archs = [target.arch, Arch::noarch];

        for depend in makedepends.as_slice() {
            let provider_idx = best_target_provider_idx(depend, &targets, &candidate_archs);
            let Some(provider_idx) = provider_idx else {
                continue;
            };
            if provider_idx == dep_idx {
                continue;
            }

            if edges[provider_idx].insert(dep_idx) {
                indegree[dep_idx] += 1;
            }
        }
    }

    let mut ready = BTreeSet::new();
    for (idx, degree) in indegree.iter().enumerate() {
        if *degree == 0 {
            ready.insert(targets[idx].pkgid.clone());
        }
    }

    let mut sorted_indices = Vec::with_capacity(targets.len());
    while let Some(pkgid) = ready.pop_first() {
        let idx = *pkgid_to_idx
            .get(&pkgid)
            .expect("ready set contains unknown package id");
        sorted_indices.push(idx);

        for dep_idx in &edges[idx] {
            indegree[*dep_idx] -= 1;
            if indegree[*dep_idx] == 0 {
                ready.insert(targets[*dep_idx].pkgid.clone());
            }
        }
    }

    if sorted_indices.len() != targets.len() {
        let mut cycle_pkgids = targets
            .iter()
            .enumerate()
            .filter(|(idx, _)| indegree[*idx] > 0)
            .map(|(_, target)| target.pkgid.to_string())
            .collect::<Vec<_>>();
        cycle_pkgids.sort_unstable();
        return Err(Err::BuildDependencyCycle(cycle_pkgids.join(", ")));
    }

    let mut targets_by_index = targets.into_iter().map(Some).collect::<Vec<_>>();
    let mut sorted = Vec::with_capacity(targets_by_index.len());
    for idx in sorted_indices {
        sorted.push(
            targets_by_index[idx]
                .take()
                .expect("build target index encountered twice"),
        );
    }
    Ok(sorted)
}

fn best_target_provider_idx(
    depend: &Depend,
    targets: &[BuildTarget],
    archs: &[Arch],
) -> Option<usize> {
    targets
        .iter()
        .enumerate()
        .filter(|(_, target)| depend.provided_by(&target.pkgid))
        .fold(
            None,
            |best: Option<(usize, &PkgId)>, (idx, target)| match best {
                None => Some((idx, &target.pkgid)),
                Some((best_idx, best_pkgid)) => {
                    match best_pkgid.better_match_than(&target.pkgid, archs) {
                        std::cmp::Ordering::Greater | std::cmp::Ordering::Equal => {
                            Some((best_idx, best_pkgid))
                        }
                        std::cmp::Ordering::Less => Some((idx, &target.pkgid)),
                    }
                }
            },
        )
        .map(|(idx, _)| idx)
}

#[cfg(test)]
mod tests {
    use crate::{error::Err, file::*, metadata::*, reconcile::*, testutil::unit_test_tmp_dir};
    use camino::{Utf8Path, Utf8PathBuf};
    use std::fs::File;

    fn write_test_bbuild(
        dir: &Utf8Path,
        filename: &str,
        pkgname: &str,
        makedepends: &str,
    ) -> Utf8PathBuf {
        let path = dir.join(filename);
        let content = format!(
            r#"#!/bin/sh
pkgname="{pkgname}"
pkgver="1.0.0"
pkgdesc="test package"
homepage="N/A"
license="MIT"
depends=""
backup=""
makearch="noarch"
makedepends="{makedepends}"
source=""
sha256sums=""

build() {{
	:
}}
"#,
        );
        std::fs::write(&path, content).unwrap();
        path
    }

    fn load_bbuild(path: &Utf8Path) -> Bbuild {
        Bbuild::from_file(
            File::open(path).unwrap(),
            &PublicKeys::from_skipping_verification(),
            None,
        )
        .unwrap()
    }

    #[test]
    fn sort_build_targets_orders_provider_before_consumer() {
        let dir = unit_test_tmp_dir(
            "build_order",
            "sort_build_targets_orders_provider_before_consumer",
        );
        let provider_path = write_test_bbuild(&dir, "zzz-helper.bbuild", "zzz-helper", "");
        let consumer_path = write_test_bbuild(
            &dir,
            "aaa-consumer.bbuild",
            "aaa-consumer",
            "zzz-helper>=1.0.0",
        );
        let provider = load_bbuild(&provider_path);
        let consumer = load_bbuild(&consumer_path);

        let sorted = sort_build_targets(vec![
            BuildTarget {
                pkgid: consumer.pkgid().with_arch(Arch::noarch),
                bbuild: &consumer,
                arch: Arch::noarch,
            },
            BuildTarget {
                pkgid: provider.pkgid().with_arch(Arch::noarch),
                bbuild: &provider,
                arch: Arch::noarch,
            },
        ])
        .unwrap();

        let ordered = sorted
            .iter()
            .map(|target| target.pkgid.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            ordered,
            vec!["zzz-helper@1.0.0:noarch", "aaa-consumer@1.0.0:noarch"]
        );
    }

    #[test]
    fn sort_build_targets_uses_pkgid_order_for_independent_targets() {
        let dir = unit_test_tmp_dir(
            "build_order",
            "sort_build_targets_uses_pkgid_order_for_independent_targets",
        );
        let alpha_path = write_test_bbuild(&dir, "alpha.bbuild", "alpha", "");
        let beta_path = write_test_bbuild(&dir, "beta.bbuild", "beta", "");
        let alpha = load_bbuild(&alpha_path);
        let beta = load_bbuild(&beta_path);

        let sorted = sort_build_targets(vec![
            BuildTarget {
                pkgid: beta.pkgid().with_arch(Arch::noarch),
                bbuild: &beta,
                arch: Arch::noarch,
            },
            BuildTarget {
                pkgid: alpha.pkgid().with_arch(Arch::noarch),
                bbuild: &alpha,
                arch: Arch::noarch,
            },
        ])
        .unwrap();

        let ordered = sorted
            .iter()
            .map(|target| target.pkgid.to_string())
            .collect::<Vec<_>>();
        assert_eq!(ordered, vec!["alpha@1.0.0:noarch", "beta@1.0.0:noarch"]);
    }

    #[test]
    fn sort_build_targets_detects_dependency_cycles() {
        let dir = unit_test_tmp_dir(
            "build_order",
            "sort_build_targets_detects_dependency_cycles",
        );
        let alpha_path = write_test_bbuild(&dir, "alpha.bbuild", "alpha", "beta>=1.0.0");
        let beta_path = write_test_bbuild(&dir, "beta.bbuild", "beta", "alpha>=1.0.0");
        let alpha = load_bbuild(&alpha_path);
        let beta = load_bbuild(&beta_path);

        let result = sort_build_targets(vec![
            BuildTarget {
                pkgid: alpha.pkgid().with_arch(Arch::noarch),
                bbuild: &alpha,
                arch: Arch::noarch,
            },
            BuildTarget {
                pkgid: beta.pkgid().with_arch(Arch::noarch),
                bbuild: &beta,
                arch: Arch::noarch,
            },
        ]);

        let err = match result {
            Ok(_) => panic!("expected dependency cycle error"),
            Err(err) => err,
        };
        match err {
            Err::BuildDependencyCycle(msg) => {
                assert!(msg.contains("alpha@1.0.0:noarch"));
                assert!(msg.contains("beta@1.0.0:noarch"));
            }
            _ => panic!("unexpected error variant: {err:?}"),
        }
    }

    #[test]
    fn sort_build_targets_ignores_dependencies_outside_target_set() {
        let dir = unit_test_tmp_dir(
            "build_order",
            "sort_build_targets_ignores_dependencies_outside_target_set",
        );
        let alpha_path = write_test_bbuild(&dir, "alpha.bbuild", "alpha", "missing>=1.0.0");
        let beta_path = write_test_bbuild(&dir, "beta.bbuild", "beta", "");
        let alpha = load_bbuild(&alpha_path);
        let beta = load_bbuild(&beta_path);

        let sorted = sort_build_targets(vec![
            BuildTarget {
                pkgid: alpha.pkgid().with_arch(Arch::noarch),
                bbuild: &alpha,
                arch: Arch::noarch,
            },
            BuildTarget {
                pkgid: beta.pkgid().with_arch(Arch::noarch),
                bbuild: &beta,
                arch: Arch::noarch,
            },
        ])
        .unwrap();

        let ordered = sorted
            .iter()
            .map(|target| target.pkgid.to_string())
            .collect::<Vec<_>>();
        assert_eq!(ordered, vec!["alpha@1.0.0:noarch", "beta@1.0.0:noarch"]);
    }

    #[test]
    fn sort_build_targets_orders_transitive_chains() {
        let dir = unit_test_tmp_dir("build_order", "sort_build_targets_orders_transitive_chains");
        let a_path = write_test_bbuild(&dir, "a.bbuild", "a", "b>=1.0.0");
        let b_path = write_test_bbuild(&dir, "b.bbuild", "b", "c>=1.0.0");
        let c_path = write_test_bbuild(&dir, "c.bbuild", "c", "");
        let a = load_bbuild(&a_path);
        let b = load_bbuild(&b_path);
        let c = load_bbuild(&c_path);

        let sorted = sort_build_targets(vec![
            BuildTarget {
                pkgid: a.pkgid().with_arch(Arch::noarch),
                bbuild: &a,
                arch: Arch::noarch,
            },
            BuildTarget {
                pkgid: c.pkgid().with_arch(Arch::noarch),
                bbuild: &c,
                arch: Arch::noarch,
            },
            BuildTarget {
                pkgid: b.pkgid().with_arch(Arch::noarch),
                bbuild: &b,
                arch: Arch::noarch,
            },
        ])
        .unwrap();

        let ordered = sorted
            .iter()
            .map(|target| target.pkgid.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            ordered,
            vec!["c@1.0.0:noarch", "b@1.0.0:noarch", "a@1.0.0:noarch"]
        );
    }

    #[test]
    fn sort_build_targets_handles_diamond_dependencies_without_duplicates() {
        let dir = unit_test_tmp_dir(
            "build_order",
            "sort_build_targets_handles_diamond_dependencies_without_duplicates",
        );
        let a_path = write_test_bbuild(&dir, "a.bbuild", "a", "b>=1.0.0 c>=1.0.0");
        let b_path = write_test_bbuild(&dir, "b.bbuild", "b", "d>=1.0.0");
        let c_path = write_test_bbuild(&dir, "c.bbuild", "c", "d>=1.0.0");
        let d_path = write_test_bbuild(&dir, "d.bbuild", "d", "");
        let a = load_bbuild(&a_path);
        let b = load_bbuild(&b_path);
        let c = load_bbuild(&c_path);
        let d = load_bbuild(&d_path);

        let sorted = sort_build_targets(vec![
            BuildTarget {
                pkgid: a.pkgid().with_arch(Arch::noarch),
                bbuild: &a,
                arch: Arch::noarch,
            },
            BuildTarget {
                pkgid: c.pkgid().with_arch(Arch::noarch),
                bbuild: &c,
                arch: Arch::noarch,
            },
            BuildTarget {
                pkgid: d.pkgid().with_arch(Arch::noarch),
                bbuild: &d,
                arch: Arch::noarch,
            },
            BuildTarget {
                pkgid: b.pkgid().with_arch(Arch::noarch),
                bbuild: &b,
                arch: Arch::noarch,
            },
        ])
        .unwrap();

        let ordered = sorted
            .iter()
            .map(|target| target.pkgid.to_string())
            .collect::<Vec<_>>();
        assert_eq!(ordered.len(), 4);
        assert_eq!(
            ordered
                .iter()
                .filter(|pkgid| *pkgid == "d@1.0.0:noarch")
                .count(),
            1
        );

        let pos = |needle: &str| ordered.iter().position(|pkgid| pkgid == needle).unwrap();
        let d_pos = pos("d@1.0.0:noarch");
        let b_pos = pos("b@1.0.0:noarch");
        let c_pos = pos("c@1.0.0:noarch");
        let a_pos = pos("a@1.0.0:noarch");

        assert!(d_pos < b_pos);
        assert!(d_pos < c_pos);
        assert!(b_pos < a_pos);
        assert!(c_pos < a_pos);
    }
}
