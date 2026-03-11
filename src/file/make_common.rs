use crate::{constant::*, error::*, io::FileAux, location::RootDir};
use std::{
    fs::File,
    io::ErrorKind,
    os::fd::{AsFd, BorrowedFd},
};

/// Package build helper functions
///
/// This type is primarily used as an abstraction to support transparently using baked-in contents
/// if the actual file does not exist on-disk.
pub struct MakeCommon(File);

impl MakeCommon {
    pub fn from_root_path(root: &RootDir) -> Result<Self, Err> {
        let path = root.as_path().join(MAKE_COMMON_PATH);
        // This file is used by a child build processes, and thus must be nocloexec
        match File::open_nocloexec(&path) {
            Ok(file) => Ok(Self(file)),
            Err(Err::Open(_, e)) if e.kind() == ErrorKind::NotFound => {
                // If file doesn't exist, use baked-in defaults
                File::create_memfd(MAKE_COMMON_FILENAME, MAKE_COMMON_DEFAULT_CONTENTS).map(Self)
            }
            Err(e) => Err(e),
        }
    }
}

impl AsFd for MakeCommon {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

#[cfg(test)]
mod tests {
    use crate::location::RootDir;
    use crate::testutil::unit_test_tmp_dir;

    use super::*;

    #[test]
    fn from_root_path_missing_file_uses_defaults() {
        let tmp = unit_test_tmp_dir("make_common", "from_root_path_missing_file_uses_defaults");
        let root = RootDir::from_path(&tmp);

        MakeCommon::from_root_path(&root).unwrap();
    }
}
