use std::collections::HashMap;

use crate::test_support::TestEnv;

use super::instance_key;
use super::model::Scope;
use super::model::{AuthConfig, HostConfig, ProfileConfig};

fn host(user: &str) -> HostConfig {
    HostConfig {
        token: "token".into(),
        user: user.into(),
    }
}

#[test]
fn set_host_updates_default_host() {
    let _env = TestEnv::new();
    let mut config = AuthConfig::default();

    config.save_login(
        &Scope::Global,
        "https://gitbucket.example.com".into(),
        host("alice"),
    );

    assert_eq!(
        config.resolve_hostname(None, None).unwrap().as_deref(),
        Some("https://gitbucket.example.com")
    );
}

#[test]
fn default_hostname_prefers_explicit_default() {
    let _env = TestEnv::new();
    let mut config = AuthConfig {
        hosts: HashMap::new(),
        default_host: Some("https://b.example.com".into()),
        ..Default::default()
    };
    config
        .hosts
        .insert("https://a.example.com".into(), host("alice"));
    config
        .hosts
        .insert("https://b.example.com".into(), host("bob"));

    assert_eq!(
        config.resolve_hostname(None, None).unwrap().as_deref(),
        Some("https://b.example.com")
    );
}

#[test]
fn default_hostname_requires_an_explicit_default() {
    let _env = TestEnv::new();
    let mut config = AuthConfig::default();
    config
        .hosts
        .insert("https://z.example.com".into(), host("zoe"));
    config
        .hosts
        .insert("https://a.example.com".into(), host("alice"));

    assert_eq!(
        config.resolve_hostname(None, None).unwrap().as_deref(),
        None
    );
}

#[test]
fn default_hostname_prefers_environment_variable() {
    let env = TestEnv::new();
    let mut config = AuthConfig::default();
    config
        .hosts
        .insert("https://a.example.com".into(), host("alice"));

    env.set("GB_HOST", "env.example.com");

    assert_eq!(
        config.resolve_hostname(None, None).unwrap().as_deref(),
        Some("env.example.com")
    );
}

#[test]
fn instance_keys_preserve_scheme_and_remove_api_suffix() {
    assert_eq!(
        instance_key("https://gitbucket.example.com/gitbucket/api/v3", "http").unwrap(),
        "https://gitbucket.example.com/gitbucket"
    );
    assert_eq!(
        instance_key("gitbucket.example.com/gitbucket", "http").unwrap(),
        "http://gitbucket.example.com/gitbucket"
    );
    assert!(instance_key(
        "https://gitbucket.example.com/gitbucket/api/v3//api/v3",
        "https"
    )
    .is_err());
}

#[test]
fn get_host_matches_equivalent_hostnames() {
    let _env = TestEnv::new();
    let mut config = AuthConfig::default();
    config.save_login(
        &Scope::Global,
        "https://gitbucket.example.com/gitbucket".into(),
        host("alice"),
    );

    let host = config
        .credential_for_profile("gitbucket.example.com/gitbucket", None)
        .unwrap();
    assert_eq!(host.protocol(), "https");
}

#[test]
fn get_host_prefers_environment_token() {
    let env = TestEnv::new();
    let config = AuthConfig::default();

    env.set("GB_TOKEN", "env-token");

    let host = config
        .credential_for_profile("https://gitbucket.example.com/gitbucket", None)
        .unwrap();
    assert_eq!(host.host.token, "env-token");
    assert_eq!(host.protocol(), "https");
    assert_eq!(host.host.user, "");
}

#[test]
fn resolve_hostname_uses_selected_profile_default_host() {
    let _env = TestEnv::new();
    let mut config = AuthConfig::default();
    config.profiles.insert(
        "work".into(),
        ProfileConfig {
            default_host: Some("https://work.example.com".into()),
            ..Default::default()
        },
    );

    assert_eq!(
        config
            .resolve_hostname(None, Some("work"))
            .unwrap()
            .as_deref(),
        Some("https://work.example.com")
    );
}

#[test]
fn resolve_repo_uses_profile_default_repo_after_cli_and_env() {
    let _env = TestEnv::new();
    let mut config = AuthConfig::default();
    config.profiles.insert(
        "work".into(),
        ProfileConfig {
            default_repo: Some("alice/project".into()),
            ..Default::default()
        },
    );

    assert_eq!(
        config.resolve_repo(None, Some("work")).unwrap().as_deref(),
        Some("alice/project")
    );
    assert_eq!(
        config
            .resolve_repo(Some("bob/override"), Some("work"))
            .unwrap()
            .as_deref(),
        Some("bob/override")
    );
}

#[test]
fn get_host_prefers_profile_scoped_credentials() {
    let _env = TestEnv::new();
    let mut config = AuthConfig::default();
    config
        .hosts
        .insert("https://gitbucket.example.com".into(), host("global"));
    config.profiles.insert(
        "work".into(),
        ProfileConfig {
            hosts: HashMap::from([("https://gitbucket.example.com".into(), host("profile"))]),
            ..Default::default()
        },
    );

    let host = config
        .credential_for_profile("gitbucket.example.com", Some("work"))
        .unwrap();
    assert_eq!(host.host.user, "profile");
}

#[test]
fn selected_missing_profile_returns_clear_error() {
    let _env = TestEnv::new();
    let config = AuthConfig::default();

    let err = config
        .resolve_hostname(None, Some("missing"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("Profile 'missing' is not configured"));
}

#[cfg(unix)]
#[test]
fn save_uses_private_file_permissions() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use super::store::write_config_file;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");

    write_config_file(&path, "token = 'secret'\n").unwrap();

    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}
