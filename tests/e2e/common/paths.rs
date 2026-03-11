use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

static RUN_ID: LazyLock<String> = LazyLock::new(|| format!("pid-{}", std::process::id()));
static INTERNED_STRINGS: LazyLock<Mutex<HashMap<String, &'static str>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn intern_runtime_string(s: String) -> &'static str {
    let mut map = INTERNED_STRINGS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(existing) = map.get(&s) {
        return existing;
    }

    let leaked = Box::leak(s.clone().into_boxed_str()) as &'static str;
    map.insert(s, leaked);
    leaked
}

pub(crate) fn per_test_root(test_name: &str) -> String {
    format!(
        "{}/e2e/{}/{}/",
        env!("CARGO_TARGET_TMPDIR"),
        RUN_ID.as_str(),
        test_name
    )
}

pub(crate) fn common_root() -> String {
    format!(
        "{}/e2e/{}/common/",
        env!("CARGO_TARGET_TMPDIR"),
        RUN_ID.as_str()
    )
}

/// Path relative to per-test environment
///
/// Leverages `#[named]` to get the test name for the per-test directory.
#[macro_export]
macro_rules! per_test_path {
    () => {{
        $crate::e2e::common::paths::intern_runtime_string(
            $crate::e2e::common::paths::per_test_root(function_name!())
        )
    }};
    ($($arg:expr),+ $(,)?) => {{
        let mut s = $crate::e2e::common::paths::per_test_root(function_name!());
        $( s.push_str($arg); )+
        $crate::e2e::common::paths::intern_runtime_string(s)
    }};
}

#[macro_export]
macro_rules! per_test_pkg_cache {
    () => {{
        let mut s = $crate::e2e::common::paths::per_test_root(function_name!());
        s.push_str("var/cache/bpt/pkgs/");
        $crate::e2e::common::paths::intern_runtime_string(s)
    }};
    ($($arg:expr),+ $(,)?) => {{
        let mut s = $crate::e2e::common::paths::per_test_root(function_name!());
        s.push_str("var/cache/bpt/pkgs/");
        $( s.push_str($arg); )+
        $crate::e2e::common::paths::intern_runtime_string(s)
    }};
}

/// Path relative to common test environment
///
/// For example, contains the `etc/bpt/` directory and its associated configuration files.
#[macro_export]
macro_rules! common_path {
    () => {{
        $crate::e2e::common::paths::intern_runtime_string($crate::e2e::common::paths::common_root())
    }};
    ($($arg:expr),+ $(,)?) => {{
        let mut s = $crate::e2e::common::paths::common_root();
        $( s.push_str($arg); )+
        $crate::e2e::common::paths::intern_runtime_string(s)
    }};
}

/// Path relative to common test repository
#[macro_export]
macro_rules! repo_path {
    () => {{
        let mut s = $crate::e2e::common::paths::common_root();
        s.push_str("repo/");
        $crate::e2e::common::paths::intern_runtime_string(s)
    }};
    ($($arg:expr),+ $(,)?) => {{
        let mut s = $crate::e2e::common::paths::common_root();
        s.push_str("repo/");
        $( s.push_str($arg); )+
        $crate::e2e::common::paths::intern_runtime_string(s)
    }};
}

/// URL relative to common test repository.
#[macro_export]
macro_rules! repo_url {
    ($($arg:expr),*) => {{
        format!(
            "http://127.0.0.1:{}/{}",
            $crate::e2e::common::file_server::file_server_port(),
            [$($arg), *].concat()
        )
    }};
}
