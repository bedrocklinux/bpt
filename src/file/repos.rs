use crate::{constant::*, error::*, io::*, location::*, str::*};
use std::{io::ErrorKind, str::FromStr};

/// Repository configuration
pub struct Repos(Vec<IdxPathUrl>);

impl Repos {
    pub fn from_root_path(root: &RootDir) -> Result<Self, Err> {
        let repo_file_paths = match root.as_path().join(REPOS_DIR_PATH).readdir() {
            Ok(paths) => paths,
            Err(Err::ReadDir(_, e)) if e.kind() == ErrorKind::NotFound => {
                // If directory doesn't exist, treat as empty
                vec![].into_iter()
            }
            Err(e) => return Err(e),
        };

        let mut repos = Vec::new();
        for repo_file_path in repo_file_paths {
            let contents = repo_file_path.read_small_file_string()?;

            for line in contents.lines() {
                let line = line.strip_comment();
                if line.is_empty() {
                    continue;
                }
                repos.push(IdxPathUrl::from_str(line)?);
            }
        }

        Ok(Self(repos))
    }

    pub fn into_vec(self) -> Vec<IdxPathUrl> {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::location::RootDir;
    use crate::testutil::unit_test_tmp_dir;

    use super::*;

    #[test]
    fn from_root_path_missing_repo_dir_returns_empty() {
        let tmp = unit_test_tmp_dir("repos", "from_root_path_missing_repo_dir_returns_empty");
        let root = RootDir::from_path(&tmp);

        let repos = Repos::from_root_path(&root).unwrap();

        assert!(repos.into_vec().is_empty());
    }
}
