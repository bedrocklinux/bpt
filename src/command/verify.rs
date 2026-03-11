use crate::{cli::*, error::*, file::*, io::*};
use camino::Utf8PathBuf;
use std::fs::File;

pub fn verify(flags: CommonFlags, paths: Vec<Utf8PathBuf>) -> Result<String, Err> {
    let pubkeys = PublicKeys::from_common_flags(&flags)?;

    for path in paths.iter() {
        let file = File::open_ro(path)?;
        let mut bf = BoundedFile::from_file(file).loc(path)?;
        bf.verify_sig(&pubkeys).loc(path)?;
    }

    if paths.len() == 1 {
        Ok(format!("Verified {} signature", paths[0]))
    } else {
        Ok(format!("Verified all {} file signatures", paths.len()))
    }
}
