use crate::error::*;
use crate::str::absolute_path_from_cwd;
use camino::{Utf8Path, Utf8PathBuf};
use std::io::ErrorKind;

const BEDROCK_GLOBAL_PATHS: &[&str] = &[
    "/boot",
    "/dev",
    "/home",
    "/lib/modules",
    "/media",
    "/mnt",
    "/proc",
    "/root",
    "/run",
    "/sys",
    "/tmp",
];

fn bedrock_prefix_for_path(path: &Utf8Path) -> Result<Option<Utf8PathBuf>, Err> {
    let output = std::process::Command::new("brl")
        .arg("which")
        .arg(path.as_str())
        .output();

    match output {
        // Not running on Bedrock
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Err::RunBrlWhich(path.to_string(), e)),
        Ok(output) if !output.status.success() => Err(Err::RunBrlWhich(
            path.to_string(),
            std::io::Error::other(format!("brl which failed with status: {}", output.status)),
        )),
        Ok(output) => {
            let stratum = String::from_utf8(output.stdout)
                .map_err(|_| Err::BrlWhichNonUtf8(path.to_string()))?;
            let stratum = stratum.trim_end();
            if stratum == "global" {
                Ok(None)
            } else {
                Ok(Some(Utf8PathBuf::from(format!(
                    "/bedrock/strata/{stratum}"
                ))))
            }
        }
    }
}

pub(crate) fn current_bedrock_prefix() -> Result<Option<Utf8PathBuf>, Err> {
    bedrock_prefix_for_path(Utf8Path::new("/"))
}

fn should_prefix_bedrock_local_path(path: &Utf8Path) -> bool {
    path.is_absolute()
        && !path.starts_with("/bedrock/strata")
        && !BEDROCK_GLOBAL_PATHS
            .iter()
            .map(Utf8Path::new)
            .any(|global| path.starts_with(global))
}

pub(crate) fn adjust_bedrock_local_path_for_prefix(
    path: &Utf8Path,
    prefix: Option<&Utf8Path>,
) -> Utf8PathBuf {
    match prefix {
        Some(prefix) if should_prefix_bedrock_local_path(path) => {
            let suffix = path
                .strip_prefix("/")
                .expect("absolute path unexpectedly failed to strip leading slash");
            prefix.join(suffix)
        }
        _ => path.to_owned(),
    }
}

/// Some file type constructors take the path to the file, while others take the root directory of
/// the file system tree that contains the file and append the per-file-type path appropriately.
///
/// Using the same path type for both classes of constructors is error prone.  Thus, this is a
/// light wrapper around an expected root directory to minimize the possibility of such errors.
#[derive(Clone)]
pub struct RootDir(Utf8PathBuf);

impl RootDir {
    pub fn as_path(&self) -> &Utf8Path {
        &self.0
    }

    pub fn adjust_bedrock_prefix(&self) -> Result<Utf8PathBuf, Err> {
        Ok(bedrock_prefix_for_path(self.as_path())?.unwrap_or_else(|| self.as_path().into()))
    }

    pub fn from_path(path: &Utf8Path) -> Self {
        Self(path.into())
    }
}

impl std::str::FromStr for RootDir {
    type Err = Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let path = Utf8PathBuf::from(s);
        let path = absolute_path_from_cwd(path.as_path()).map_err(|e| {
            if e.kind() == ErrorKind::InvalidData {
                Err::InputFieldInvalid("root directory", e.to_string())
            } else {
                Err::Open("current working directory".to_string(), e)
            }
        })?;
        Ok(RootDir(path))
    }
}

impl std::fmt::Display for RootDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjust_bedrock_local_path_for_prefix_prefixes_local_paths() {
        let adjusted = adjust_bedrock_local_path_for_prefix(
            Utf8Path::new("/var/lib/bpt/build"),
            Some(Utf8Path::new("/bedrock/strata/bpt")),
        );
        assert_eq!(
            adjusted,
            Utf8PathBuf::from("/bedrock/strata/bpt/var/lib/bpt/build")
        );
    }

    #[test]
    fn adjust_bedrock_local_path_for_prefix_keeps_global_paths() {
        let adjusted = adjust_bedrock_local_path_for_prefix(
            Utf8Path::new("/home/bpt"),
            Some(Utf8Path::new("/bedrock/strata/bpt")),
        );
        assert_eq!(adjusted, Utf8PathBuf::from("/home/bpt"));
    }

    #[test]
    fn adjust_bedrock_local_path_for_prefix_keeps_already_prefixed_paths() {
        let adjusted = adjust_bedrock_local_path_for_prefix(
            Utf8Path::new("/bedrock/strata/bpt/var/lib/bpt/build"),
            Some(Utf8Path::new("/bedrock/strata/bpt")),
        );
        assert_eq!(
            adjusted,
            Utf8PathBuf::from("/bedrock/strata/bpt/var/lib/bpt/build")
        );
    }
}
