use crate::{make_field_list, metadata::*};

/// Instruction Set Architectures which could be built from the given package build definition
/// (`*.bbuild`).
///
/// This should never be populated within a built package (`*.bpt`).
#[derive(Clone, Debug)]
pub struct MakeArchs(Vec<Arch>);

make_field_list!(MakeArchs, PkgKey, Arch);

impl MakeArchs {
    pub fn can_build(&self, arch: Arch) -> bool {
        if arch == Arch::native {
            self.0.contains(&Arch::host())
        } else {
            self.0.contains(&arch)
        }
    }
}
