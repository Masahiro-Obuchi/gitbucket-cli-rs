use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use super::model::{AuthConfig, HostConfig};
use super::resolve::{canonical_hostname, protocol_from_hostname};
use super::store::write_config_file;

fn host(user: &str) -> HostConfig {
    HostConfig {
        token: "token".into(),
        user: user.into(),
        protocol: "https".into(),
    }
}

fn temp_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("gb-tests-{name}-{}-{nanos}", std::process::id()))
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn clear_auth_env() {
    unsafe {
        std::env::remove_var("GB_HOST");
        std::env::remove_var("GB_TOKEN");
        std::env::remove_var("GB_PROTOCOL");
    }
}

#[test]
fn set_host_updates_default_host() {
    let _guard = env_lock().lock().unwrap();
    clear_auth_env();
    let mut config = AuthConfig::default();

    config.set_host("gitbucket.example.com".into(), host("alice"));

    assert_eq!(
        config.default_hostname().as_deref(),
        Some("gitbucket.example.com")
    );
}

#[test]
fn default_hostname_prefers_explicit_default() {
    let _guard = env_lock().lock().unwrap();
    clear_auth_env();
    let mut config = AuthConfig {
        hosts: HashMap::new(),
        default_host: Some("b.example.com".into()),
    };
    config.hosts.insert("a.example.com".into(), host("alice"));
    config.hosts.insert("b.example.com".into(), host("bob"));

    assert_eq!(config.default_hostname().as_deref(), Some("b.example.com"));
}

#[test]
fn default_hostname_falls_back_to_sorted_hostnames() {
    let _guard = env_lock().lock().unwrap();
    clear_auth_env();
    let mut config = AuthConfig::default();
    config.hosts.insert("z.example.com".into(), host("zoe"));
    config.hosts.insert("a.example.com".into(), host("alice"));

    assert_eq!(config.default_hostname().as_deref(), Some("a.example.com"));
}

#[test]
fn default_hostname_prefers_environment_variable() {
    let _guard = env_lock().lock().unwrap();
    clear_auth_env();
    let mut config = AuthConfig::default();
    config.hosts.insert("a.example.com".into(), host("alice"));

    unsafe {
        std::env::set_var("GB_HOST", "env.example.com");
    }

    assert_eq!(
        config.default_hostname().as_deref(),
        Some("env.example.com")
    );

    unsafe {
        std::env::remove_var("GB_HOST");
    }
}

#[test]
fn protocol_can_be_derived_from_hostname() {
    assert_eq!(
        protocol_from_hostname("http://localhost:8080/gitbucket").as_deref(),
        Some("http")
    );
    assert_eq!(
        protocol_from_hostname("https://gitbucket.example.com").as_deref(),
        Some("https")
    );
    assert_eq!(protocol_from_hostname("gitbucket.example.com"), None);
}

#[test]
fn canonical_hostname_ignores_scheme_and_api_suffix() {
    assert_eq!(
        canonical_hostname("https://gitbucket.example.com/gitbucket/api/v3").as_deref(),
        Some("gitbucket.example.com/gitbucket")
    );
    assert_eq!(
        canonical_hostname("gitbucket.example.com/gitbucket").as_deref(),
        Some("gitbucket.example.com/gitbucket")
    );
}

#[test]
fn get_host_matches_equivalent_hostnames() {
    let _guard = env_lock().lock().unwrap();
    clear_auth_env();
    let mut config = AuthConfig::default();
    config.set_host(
        "https://gitbucket.example.com/gitbucket".into(),
        HostConfig {
            token: "token".into(),
            user: "alice".into(),
            protocol: "http".into(),
        },
    );

    let host = config.get_host("gitbucket.example.com/gitbucket").unwrap();
    assert_eq!(host.protocol, "http");
}

#[test]
fn get_host_prefers_environment_token() {
    let _guard = env_lock().lock().unwrap();
    clear_auth_env();
    let config = AuthConfig::default();

    unsafe {
        std::env::set_var("GB_TOKEN", "env-token");
    }

    let host = config
        .get_host("https://gitbucket.example.com/gitbucket")
        .unwrap();
    assert_eq!(host.token, "env-token");
    assert_eq!(host.protocol, "https");
    assert_eq!(host.user, "");

    unsafe {
        std::env::remove_var("GB_TOKEN");
    }
}

#[test]
fn remove_host_promotes_next_sorted_host() {
    let _guard = env_lock().lock().unwrap();
    clear_auth_env();
    let mut config = AuthConfig {
        hosts: HashMap::new(),
        default_host: Some("b.example.com".into()),
    };
    config.hosts.insert("a.example.com".into(), host("alice"));
    config.hosts.insert("b.example.com".into(), host("bob"));

    assert!(config.remove_host("b.example.com"));
    assert_eq!(config.default_hostname().as_deref(), Some("a.example.com"));
}

#[cfg(unix)]
#[test]
fn save_uses_private_file_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_path("config-dir");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");

    write_config_file(&path, "token = 'secret'\n").unwrap();

    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);

    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(&dir);
}
