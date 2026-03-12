use crate::{
    constant::*,
    error::*,
    io::*,
    location::{RootDir, adjust_bedrock_local_path_for_prefix, current_bedrock_prefix},
    metadata::*,
    str::*,
};
use camino::Utf8PathBuf;
use nix::unistd::{Group, User, geteuid};
use std::{io::ErrorKind, str::FromStr};

/// bpt configuration
///
/// Organized by sections
#[derive(Debug, PartialEq)]
pub struct BptConf {
    pub general: BptConfGeneral,
    pub build: BptConfBuild,
    pub make_repo: BptConfMakeRepo,
    pub networking: BptConfNetworking,
    pub cache: BptConfCache,
}

#[derive(Debug, PartialEq)]
pub struct BptConfGeneral {
    pub default_archs: Vec<Arch>,
    pub pin_direct_pkgver: bool,
}

#[derive(Debug, PartialEq)]
pub struct BptConfBuild {
    pub unprivileged_user: String,
    pub unprivileged_group: String,
    pub tmp: Utf8PathBuf,
}

#[derive(Debug, PartialEq)]
pub struct BptConfMakeRepo {
    pub archs: Vec<Arch>,
}

#[derive(Debug, PartialEq)]
pub struct BptConfNetworking {
    pub utils: Vec<NetworkingUtil>,
    pub print_stderr: bool,
}

#[derive(Debug, PartialEq)]
pub struct NetworkingUtil(Vec<String>);

#[derive(Debug, PartialEq)]
pub struct BptConfCache {
    pub cache_pkg_max_days: Option<u32>,
    pub cache_src_max_days: Option<u32>,
}

enum Section {
    Preamble, // before any section header
    Build,
    General,
    MakeRepo,
    Networking,
    Cache,
}

impl FromStr for Section {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "[general]" => Ok(Section::General),
            "[build]" => Ok(Section::Build),
            "[make-repo]" => Ok(Section::MakeRepo),
            "[networking]" => Ok(Section::Networking),
            "[cache]" => Ok(Section::Cache),
            _ => Err("section header"),
        }
    }
}

impl BptConfGeneral {
    pub fn update(&mut self, key: &str, value: &str) -> Result<(), &'static str> {
        match key {
            "default-archs" => {
                self.default_archs = value.parse_arch_list()?;
            }
            "pin-direct-pkgver" => {
                self.pin_direct_pkgver = value.parse_bool()?;
            }
            _ => return Err("[general] key"),
        }
        Ok(())
    }
}

impl BptConfBuild {
    pub fn update(&mut self, key: &str, value: &str) -> Result<(), &'static str> {
        match key {
            "unprivileged-user" => self.unprivileged_user = value.into(),
            "unprivileged-group" => self.unprivileged_group = value.into(),
            "tmp" => self.tmp = Utf8PathBuf::from_str(value).map_err(|_| "file path")?,
            _ => return Err("[build] key"),
        }
        Ok(())
    }
}

impl BptConfMakeRepo {
    pub fn update(&mut self, key: &str, value: &str) -> Result<(), &'static str> {
        match key {
            "archs" => self.archs = value.parse_arch_list()?,
            _ => return Err("[make-repo] key"),
        }
        Ok(())
    }
}

impl BptConfNetworking {
    pub fn update(&mut self, key: &str, value: &str) -> Result<(), &'static str> {
        match key {
            "util" => {
                if !value.split_ascii_whitespace().any(|s| s == "{}") {
                    return Err("netutil description (missing `{}`)");
                }
                let terms = value.split_ascii_whitespace().map(String::from).collect();
                self.utils.push(NetworkingUtil(terms));
            }
            "print-stderr" => self.print_stderr = value.parse_bool()?,
            _ => return Err("[networking] key"),
        }
        Ok(())
    }
}

impl BptConfCache {
    pub fn update(&mut self, key: &str, value: &str) -> Result<(), &'static str> {
        match key {
            "pkg-max-days" => self.cache_pkg_max_days = parse_max_days(value)?,
            "src-max-days" => self.cache_src_max_days = parse_max_days(value)?,
            _ => return Err("[cache] key"),
        }
        Ok(())
    }
}

/// Parse a max-days value: "forever" → None, non-negative integer → Some(n)
fn parse_max_days(value: &str) -> Result<Option<u32>, &'static str> {
    if value == "forever" {
        return Ok(None);
    }
    value
        .parse::<u32>()
        .map(Some)
        .map_err(|_| "non-negative integer or \"forever\"")
}

impl BptConf {
    pub(crate) fn build_credentials_for_euid(
        &self,
        euid: nix::unistd::Uid,
    ) -> Result<Option<ProcessCredentials>, Err> {
        if !euid.is_root() {
            return Ok(None);
        }

        let user_name = self.build.unprivileged_user.clone();
        let group_name = self.build.unprivileged_group.clone();

        let user = User::from_name(&user_name)
            .map_err(|e| Err::InputFieldInvalid("build unprivileged-user", e.to_string()))?
            .ok_or_else(|| {
                Err::InputFieldInvalid(
                    "build unprivileged-user",
                    format!("user `{user_name}` was not found"),
                )
            })?;
        let group = Group::from_name(&group_name)
            .map_err(|e| Err::InputFieldInvalid("build unprivileged-group", e.to_string()))?
            .ok_or_else(|| {
                Err::InputFieldInvalid(
                    "build unprivileged-group",
                    format!("group `{group_name}` was not found"),
                )
            })?;
        let home_dir = Utf8PathBuf::from_path_buf(user.dir.clone()).map_err(|path| {
            Err::InputFieldInvalid(
                "build unprivileged-user",
                format!(
                    "user `{user_name}` has a non-UTF-8 home directory `{}`",
                    path.display()
                ),
            )
        })?;
        let home_dir =
            adjust_bedrock_local_path_for_prefix(&home_dir, current_bedrock_prefix()?.as_deref());

        Ok(Some(ProcessCredentials {
            user_name,
            uid: user.uid,
            gid: group.gid,
            home_dir: home_dir.into_std_path_buf(),
        }))
    }

    pub fn build_credentials(&self) -> Result<Option<ProcessCredentials>, Err> {
        self.build_credentials_for_euid(geteuid())
    }

    pub fn from_root_path(root: &RootDir) -> Result<Self, Err> {
        let path = root.as_path().join(BPT_CONF_PATH);
        let contents = match path.read_small_file_string() {
            Ok(contents) => contents,
            Err(Err::Open(_, e)) if e.kind() == ErrorKind::NotFound => {
                // If file doesn't exist; use hard-coded defaults
                return Ok(Self::default());
            }
            Err(e) => Err(e)?,
        };

        Self::from_file_contents(&contents).loc(path)
    }

    pub fn from_file_contents(contents: &str) -> Result<Self, AnonLocErr> {
        // If any items are unspecified, fall back to the hard-coded default.
        let mut conf = Self::default();

        let mut section = Section::Preamble;

        // Parse networking into a separate struct starting empty; if any utils are specified,
        // they replace the defaults.
        let mut networking = BptConfNetworking {
            utils: Vec::new(),
            print_stderr: conf.networking.print_stderr,
        };

        for (nr, line) in contents.lines().enumerate().map(|(nr, l)| (nr + 1, l)) {
            let line = line.strip_comment();
            if line.is_empty() {
                continue;
            }

            if line.starts_with('[') {
                section = Section::from_str(line)
                    .map_err(|invalid| AnonLocErr::BptConfInvalidLine(nr, invalid, line.into()))?;
                continue;
            }

            let (key, value) = line
                .split_once('=')
                .map(|(key, value)| (key.trim(), value.trim()))
                .ok_or_else(|| {
                    AnonLocErr::BptConfInvalidLine(nr, "\"key = value\" pair", line.into())
                })?;

            match section {
                Section::Preamble => {
                    return Err(AnonLocErr::BptConfInvalidLine(
                        nr,
                        "line before any [section] header",
                        line.into(),
                    ));
                }
                Section::Build => conf.build.update(key, value),
                Section::General => conf.general.update(key, value),
                Section::MakeRepo => conf.make_repo.update(key, value),
                Section::Networking => networking.update(key, value),
                Section::Cache => conf.cache.update(key, value),
            }
            .map_err(|invalid| AnonLocErr::BptConfInvalidLine(nr, invalid, line.to_string()))?;
        }

        // If the config specified any utils, use those instead of the defaults.
        if !networking.utils.is_empty() {
            conf.networking.utils = networking.utils;
        }
        conf.networking.print_stderr = networking.print_stderr;

        Ok(conf)
    }
}

impl Default for BptConf {
    fn default() -> Self {
        Self {
            general: BptConfGeneral {
                default_archs: vec![Arch::noarch, Arch::host()],
                pin_direct_pkgver: false,
            },
            build: BptConfBuild {
                unprivileged_user: "bpt".to_owned(),
                unprivileged_group: "bpt".to_owned(),
                tmp: Utf8PathBuf::from("/tmp"),
            },
            make_repo: BptConfMakeRepo {
                archs: vec![Arch::noarch, Arch::host(), Arch::bbuild],
            },
            networking: BptConfNetworking {
                utils: vec![
                    NetworkingUtil(vec!["curl".into(), "-f".into(), "-L".into(), "{}".into()]),
                    NetworkingUtil(vec!["wget".into(), "-O-".into(), "{}".into()]),
                ],
                print_stderr: false,
            },
            cache: BptConfCache {
                cache_pkg_max_days: Some(90),
                cache_src_max_days: Some(90),
            },
        }
    }
}

trait ParseBool {
    fn parse_bool(&self) -> Result<bool, &'static str>;
}

impl ParseBool for str {
    fn parse_bool(&self) -> Result<bool, &'static str> {
        match self {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err("boolean value"),
        }
    }
}

trait ParseArchList {
    fn parse_arch_list(&self) -> Result<Vec<Arch>, &'static str>;
}

impl ParseArchList for str {
    fn parse_arch_list(&self) -> Result<Vec<Arch>, &'static str> {
        self.split(',')
            .map(|field| field.trim())
            .map(|arch| Arch::try_from(arch).map_err(|_| "arch"))
            .collect()
    }
}

impl NetworkingUtil {
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::RootDir;
    use crate::testutil::unit_test_tmp_dir;
    use camino::Utf8PathBuf;
    use nix::unistd::Uid;

    #[test]
    fn test_missing_root_config_uses_defaults() {
        let tmp = unit_test_tmp_dir("bpt_conf", "test_missing_root_config_uses_defaults");
        let root = RootDir::from_path(&tmp);

        let conf = BptConf::from_root_path(&root).unwrap();

        assert_eq!(conf, BptConf::default());
    }

    #[test]
    fn test_default_config_file_matches_default_impl() {
        let from_file = BptConf::from_file_contents(
            std::str::from_utf8(include_bytes!("../../assets/default-configs/bpt.conf")).unwrap(),
        )
        .expect("Failed to parse default bpt.conf");

        assert_eq!(from_file, BptConf::default());
    }

    #[test]
    fn test_build_credentials_returns_none_for_non_root() {
        let conf = BptConf::default();
        let creds = conf
            .build_credentials_for_euid(Uid::from_raw(1000))
            .unwrap();
        assert_eq!(creds, None);
    }

    #[test]
    fn test_parse_valid_full_config() {
        let config_str = r#"
            [general]
            default-archs = x86_64, aarch64

            [build]
            unprivileged-user = builder
            unprivileged-group = builders
            tmp = /var/tmp

            [make-repo]
            archs = noarch, host

            [networking]
            util = curl -f -L {}
            util = wget -O- {}
            print-stderr = true

            [cache]
            pkg-max-days = 30
            src-max-days = forever
        "#;

        let parsed_conf =
            BptConf::from_file_contents(config_str).expect("Failed to parse valid full config");

        let expected_conf = BptConf {
            general: BptConfGeneral {
                default_archs: vec![Arch::x86_64, Arch::aarch64],
                pin_direct_pkgver: false,
            },
            build: BptConfBuild {
                unprivileged_user: "builder".to_owned(),
                unprivileged_group: "builders".to_owned(),
                tmp: Utf8PathBuf::from("/var/tmp"),
            },
            make_repo: BptConfMakeRepo {
                archs: vec![Arch::noarch, Arch::host()],
            },
            networking: BptConfNetworking {
                utils: vec![
                    NetworkingUtil(vec!["curl".into(), "-f".into(), "-L".into(), "{}".into()]),
                    NetworkingUtil(vec!["wget".into(), "-O-".into(), "{}".into()]),
                ],
                print_stderr: true,
            },
            cache: BptConfCache {
                cache_pkg_max_days: Some(30),
                cache_src_max_days: None,
            },
        };

        assert_eq!(&parsed_conf, &expected_conf);
    }

    #[test]
    fn test_parse_missing_sections() {
        let config_str = r#"
            [general]
            default-archs = x86_64

            [build]
            unprivileged-user = builder
        "#;

        let parsed_conf = BptConf::from_file_contents(config_str)
            .expect("Failed to parse config with missing sections");

        let mut expected_conf = BptConf::default();
        expected_conf.general.default_archs = vec![Arch::x86_64];
        expected_conf.build.unprivileged_user = "builder".to_owned();
        expected_conf.build.unprivileged_group = "bpt".to_owned();
        expected_conf.build.tmp = Utf8PathBuf::from("/tmp");
        // make_repo, networking, and cache should remain as defaults.

        assert_eq!(&parsed_conf, &expected_conf);
    }

    #[test]
    fn test_parse_invalid_section_header() {
        let config_str = r#"
            [general]
            default-archs = x86_64

            [invalid-section]
            key = value
        "#;

        let result = BptConf::from_file_contents(config_str);
        assert!(result.is_err());

        match result {
            Err(AnonLocErr::BptConfInvalidLine(nr, invalid, line)) => {
                assert_eq!(nr, 5);
                assert_eq!(invalid, "section header");
                assert_eq!(line, "[invalid-section]");
            }
            _ => panic!("Unexpected error variant"),
        }
    }

    #[test]
    fn test_parse_invalid_key_in_section() {
        let config_str = r#"
            [build]
            unprivileged-user = builder
            invalid-key = value
        "#;

        let result = BptConf::from_file_contents(config_str);
        assert!(result.is_err());

        match result {
            Err(AnonLocErr::BptConfInvalidLine(nr, invalid, line)) => {
                assert_eq!(nr, 4);
                assert_eq!(invalid, "[build] key");
                assert_eq!(line, "invalid-key = value");
            }
            _ => panic!("Unexpected error variant"),
        }
    }

    #[test]
    fn test_parse_invalid_value_type() {
        let config_str = r#"
            [cache]
            pkg-max-days = not_an_integer
        "#;

        let result = BptConf::from_file_contents(config_str);
        assert!(result.is_err());

        match result {
            Err(AnonLocErr::BptConfInvalidLine(nr, invalid, line)) => {
                assert_eq!(nr, 3);
                assert_eq!(invalid, "non-negative integer or \"forever\"");
                assert_eq!(line, "pkg-max-days = not_an_integer");
            }
            _ => panic!("Unexpected error variant"),
        }
    }

    #[test]
    fn test_parse_missing_key_value_separator() {
        let config_str = r#"
            [general]
            default-archs x86_64, aarch64
        "#;

        let result = BptConf::from_file_contents(config_str);
        assert!(result.is_err());

        match result {
            Err(AnonLocErr::BptConfInvalidLine(nr, invalid, line)) => {
                assert_eq!(nr, 3);
                assert_eq!(invalid, "\"key = value\" pair");
                assert_eq!(line, "default-archs x86_64, aarch64");
            }
            _ => panic!("Unexpected error variant"),
        }
    }

    #[test]
    fn test_parse_invalid_boolean() {
        let config_str = r#"
            [networking]
            print-stderr = not_a_bool
        "#;

        let result = BptConf::from_file_contents(config_str);
        assert!(result.is_err());

        match result {
            Err(AnonLocErr::BptConfInvalidLine(nr, invalid, line)) => {
                assert_eq!(nr, 3);
                assert_eq!(invalid, "boolean value");
                assert_eq!(line, "print-stderr = not_a_bool");
            }
            _ => panic!("Unexpected error variant"),
        }
    }

    #[test]
    fn test_parse_with_whitespace_and_comments() {
        let config_str = r#"
            # This is a comment
            [general]
            default-archs = x86_64,    aarch64  # Inline comment

            [build]   # Another comment
            unprivileged-user = builder
            unprivileged-group = builders

            [cache]
            pkg-max-days = 45
        "#;

        let parsed_conf = BptConf::from_file_contents(config_str)
            .expect("Failed to parse config with whitespace and comments");

        let mut expected_conf = BptConf::default();
        expected_conf.general.default_archs = vec![Arch::x86_64, Arch::aarch64];
        expected_conf.build.unprivileged_user = "builder".to_owned();
        expected_conf.build.unprivileged_group = "builders".to_owned();
        expected_conf.cache.cache_pkg_max_days = Some(45);
        // Other fields should remain as defaults.

        assert_eq!(&parsed_conf, &expected_conf);
    }

    #[test]
    fn test_parse_only_comments_and_empty_lines() {
        let config_str = r#"
            # Entire configuration is comments and empty lines

            # Another comment

        "#;

        let parsed_conf = BptConf::from_file_contents(config_str)
            .expect("Failed to parse config with only comments and empty lines");

        let expected_conf = BptConf::default();

        assert_eq!(&parsed_conf, &expected_conf);
    }

    #[test]
    fn test_parse_duplicate_keys() {
        let config_str = r#"
            [build]
            unprivileged-user = builder1
            unprivileged-user = builder2
            tmp = /tmp1
            tmp = /tmp2
        "#;

        let parsed_conf = BptConf::from_file_contents(config_str)
            .expect("Failed to parse config with duplicate keys");

        let mut expected_conf = BptConf::default();
        expected_conf.build.unprivileged_user = "builder2".to_owned();
        expected_conf.build.tmp = Utf8PathBuf::from("/tmp2");
        // Other fields should remain as defaults.

        assert_eq!(&parsed_conf, &expected_conf);
    }

    #[test]
    fn test_parse_networking_utils_missing_placeholder() {
        let config_str = r#"
            [networking]
            util = curl -f -L
        "#;

        let result = BptConf::from_file_contents(config_str);
        assert!(result.is_err());

        match result {
            Err(AnonLocErr::BptConfInvalidLine(nr, invalid, line)) => {
                assert_eq!(nr, 3);
                assert_eq!(invalid, "netutil description (missing `{}`)");
                assert_eq!(line, "util = curl -f -L");
            }
            _ => panic!("Unexpected error variant"),
        }
    }

    #[test]
    fn test_parse_invalid_arch_in_general() {
        let config_str = r#"
            [general]
            default-archs = x86_64, invalid_arch
        "#;

        let result = BptConf::from_file_contents(config_str);
        assert!(result.is_err());

        match result {
            Err(AnonLocErr::BptConfInvalidLine(nr, invalid, line)) => {
                assert_eq!(nr, 3);
                assert_eq!(invalid, "arch");
                assert_eq!(line, "default-archs = x86_64, invalid_arch");
            }
            _ => panic!("Unexpected error variant"),
        }
    }

    #[test]
    fn test_parse_empty_configuration() {
        let config_str = "";

        let parsed_conf =
            BptConf::from_file_contents(config_str).expect("Failed to parse empty configuration");

        let expected_conf = BptConf::default();

        assert_eq!(&parsed_conf, &expected_conf);
    }

    #[test]
    fn test_parse_only_one_section() {
        let config_str = r#"
            [cache]
            pkg-max-days = 60
            src-max-days = 60
        "#;

        let parsed_conf = BptConf::from_file_contents(config_str)
            .expect("Failed to parse config with only cache section");

        let mut expected_conf = BptConf::default();
        expected_conf.cache.cache_pkg_max_days = Some(60);
        expected_conf.cache.cache_src_max_days = Some(60);
        // Other fields should remain as defaults.

        assert_eq!(&parsed_conf, &expected_conf);
    }

    #[test]
    fn test_parse_cache_forever() {
        let config_str = r#"
            [cache]
            pkg-max-days = forever
            src-max-days = forever
        "#;

        let parsed_conf =
            BptConf::from_file_contents(config_str).expect("Failed to parse cache forever");

        assert_eq!(parsed_conf.cache.cache_pkg_max_days, None);
        assert_eq!(parsed_conf.cache.cache_src_max_days, None);
    }

    #[test]
    fn test_parse_cache_zero() {
        let config_str = r#"
            [cache]
            pkg-max-days = 0
            src-max-days = 0
        "#;

        let parsed_conf =
            BptConf::from_file_contents(config_str).expect("Failed to parse cache zero");

        assert_eq!(parsed_conf.cache.cache_pkg_max_days, Some(0));
        assert_eq!(parsed_conf.cache.cache_src_max_days, Some(0));
    }

    #[test]
    fn test_parse_cache_negative_rejected() {
        let config_str = r#"
            [cache]
            pkg-max-days = -1
        "#;

        let result = BptConf::from_file_contents(config_str);
        assert!(result.is_err());

        match result {
            Err(AnonLocErr::BptConfInvalidLine(nr, invalid, line)) => {
                assert_eq!(nr, 3);
                assert_eq!(invalid, "non-negative integer or \"forever\"");
                assert_eq!(line, "pkg-max-days = -1");
            }
            _ => panic!("Unexpected error variant"),
        }
    }

    #[test]
    fn test_parse_networking_single_util() {
        let config_str = r#"
            [networking]
            util = curl -f -L {}
        "#;

        let parsed_conf = BptConf::from_file_contents(config_str)
            .expect("Failed to parse networking with single util");

        let mut expected_conf = BptConf::default();
        expected_conf.networking.utils = vec![NetworkingUtil(vec![
            "curl".into(),
            "-f".into(),
            "-L".into(),
            "{}".into(),
        ])];
        // Other fields should remain as defaults.

        assert_eq!(&parsed_conf, &expected_conf);
    }

    #[test]
    fn test_parse_networking_no_utils() {
        let config_str = r#"
            [networking]
            print-stderr = true
        "#;

        let parsed_conf = BptConf::from_file_contents(config_str)
            .expect("Failed to parse networking with no utils");

        let mut expected_conf = BptConf::default();
        expected_conf.networking.print_stderr = true;
        // utils should remain as defaults.

        assert_eq!(&parsed_conf, &expected_conf);
    }
}
