mod support;

use serde_json::Value;
use std::{fs, process::Command};
use support::gb_cmd::{assert_failure, assert_success, GbTestEnv};
use support::mock_http::{spawn_scripted_server, ScriptedResponse};

fn command(env: &GbTestEnv) -> Command {
    let mut cmd = env.command();
    for key in [
        "GB_TOKEN",
        "GB_PROFILE",
        "GB_PROTOCOL",
        "GB_HOST",
        "GB_USER",
    ] {
        cmd.env_remove(key);
    }
    cmd
}

const SCOPED: &str = r#"
default_profile = "work"
default_host = "https://bucket.test/gitbucket"
[hosts."https://bucket.test/gitbucket"]
token = "global-secret"
user = "global-user"
[profiles.work]
default_host = "https://bucket.test/gitbucket"
default_repo = "work/project"
[profiles.work.hosts."https://bucket.test/gitbucket"]
token = "work-secret"
user = "work-user"
[profiles.personal]
default_host = "https://bucket.test/gitbucket"
[profiles.personal.hosts."https://bucket.test/gitbucket"]
token = "personal-secret"
user = "personal-user"
"#;

#[test]
fn config_host_selection_respects_the_selected_profile() {
    let env = GbTestEnv::new();
    fs::write(env.path().join("config.toml"), SCOPED).unwrap();
    let out = command(&env)
        .args([
            "config",
            "get",
            "host",
            "--host",
            "https://bucket.test/gitbucket",
            "--field",
            "user",
        ])
        .output()
        .unwrap();
    assert_success(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "work-user");
}

#[test]
fn profile_logout_preserves_fallback_global_credentials() {
    let env = GbTestEnv::new();
    let original = SCOPED.replace("[profiles.work.hosts.\"https://bucket.test/gitbucket\"]\ntoken = \"work-secret\"\nuser = \"work-user\"\n", "");
    fs::write(env.path().join("config.toml"), &original).unwrap();
    let out = command(&env)
        .args(["auth", "logout", "--profile", "work"])
        .output()
        .unwrap();
    assert_success(&out);
    assert_eq!(
        fs::read_to_string(env.path().join("config.toml")).unwrap(),
        original
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("global"));
}

#[test]
fn explicit_http_never_selects_a_saved_https_token() {
    let env = GbTestEnv::new();
    fs::write(env.path().join("config.toml"), SCOPED).unwrap();
    let out = command(&env)
        .args([
            "auth",
            "token",
            "--hostname",
            "http://bucket.test/gitbucket",
        ])
        .output()
        .unwrap();
    assert_failure(&out);
    assert!(out.stdout.is_empty());
}

#[test]
fn scoped_edits_global_override_and_profile_cleanup_are_isolated() {
    let env = GbTestEnv::new();
    let path = env.path().join("config.toml");
    fs::write(&path, SCOPED).unwrap();
    for (args, scope, user) in [
        (
            vec![
                "config",
                "set",
                "host",
                "--host",
                "https://bucket.test/gitbucket",
                "--user",
                "edited-work",
            ],
            "work",
            "edited-work",
        ),
        (
            vec![
                "config",
                "set",
                "host",
                "--host",
                "https://bucket.test/gitbucket",
                "--user",
                "edited-global",
                "--global",
            ],
            "global",
            "edited-global",
        ),
    ] {
        let out = command(&env).args(args).output().unwrap();
        assert_success(&out);
        let value: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let table = if scope == "global" {
            &value
        } else {
            &value["profiles"][scope]
        };
        assert_eq!(
            table["hosts"]["https://bucket.test/gitbucket"]["user"].as_str(),
            Some(user)
        );
        assert_eq!(
            value["profiles"]["personal"]["hosts"]["https://bucket.test/gitbucket"]["token"]
                .as_str(),
            Some("personal-secret")
        );
    }
    for args in [
        vec!["config", "unset", "default-repo"],
        vec!["config", "unset", "default-host"],
        vec![
            "config",
            "unset",
            "host",
            "--host",
            "https://bucket.test/gitbucket",
        ],
        vec!["config", "unset", "profile", "work"],
    ] {
        assert_success(&command(&env).args(args).output().unwrap());
    }
    let value: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(value.get("default_profile").is_none());
    assert!(value["profiles"].get("work").is_none());
    assert_eq!(
        value["hosts"]["https://bucket.test/gitbucket"]["token"].as_str(),
        Some("global-secret")
    );
    assert!(value["profiles"].get("personal").is_some());
}

#[test]
fn explicit_global_and_profile_conflict_before_mutation() {
    let env = GbTestEnv::new();
    let path = env.path().join("config.toml");
    fs::write(&path, SCOPED).unwrap();
    for args in [
        vec!["auth", "logout", "--global", "--profile", "work"],
        vec![
            "config",
            "unset",
            "default-host",
            "--global",
            "--profile",
            "work",
        ],
    ] {
        assert_failure(&command(&env).args(args).output().unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), SCOPED);
    }
    assert_failure(
        &command(&env)
            .env("GB_PROFILE", "personal")
            .args(["auth", "logout", "--global"])
            .output()
            .unwrap(),
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), SCOPED);
}

#[test]
fn own_profile_logout_retains_target_and_explains_remaining_credentials() {
    let env = GbTestEnv::new();
    let path = env.path().join("config.toml");
    fs::write(&path, SCOPED).unwrap();
    let out = command(&env)
        .env("GB_TOKEN", "env-secret")
        .args(["auth", "logout"])
        .output()
        .unwrap();
    assert_success(&out);
    assert!(String::from_utf8_lossy(&out.stdout).contains("GB_TOKEN remains active"));
    let value: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        value["profiles"]["work"]["default_host"].as_str(),
        Some("https://bucket.test/gitbucket")
    );
    assert!(value["profiles"]["work"]["hosts"]
        .as_table()
        .unwrap()
        .is_empty());
    let out = command(&env).args(["auth", "token"]).output().unwrap();
    assert_success(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "global-secret");
}

#[test]
fn status_probes_the_token_instead_of_trusting_saved_or_environment_user_hints() {
    for (saved, status, body, expected) in [
        (false, "200 OK", r#"{"login":"server-user"}"#, "verified"),
        (true, "200 OK", r#"{"login":"server-user"}"#, "verified"),
        (
            false,
            "401 Unauthorized",
            r#"{"message":"token rejected"}"#,
            "rejected",
        ),
        (false, "500 Internal Server Error", "{}", "unverified"),
    ] {
        let env = GbTestEnv::new();
        let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
            "GET /gitbucket/api/v3/user HTTP/1.1",
            status,
            body,
        )]);
        let target = format!("http://127.0.0.1:{port}/gitbucket");
        if saved {
            fs::write(env.path().join("config.toml"), format!("default_host = '{target}'\n[hosts.'{target}']\ntoken = 'saved-secret'\nuser = 'stale-user'\nprotocol = 'http'\n")).unwrap();
        }
        let mut cmd = command(&env);
        cmd.env("GB_HOST", &target).env("GB_USER", "untrusted-hint");
        if !saved {
            cmd.env("GB_TOKEN", "env-secret");
        }
        let out = cmd.args(["auth", "status", "--json"]).output().unwrap();
        if status == "200 OK" {
            assert_success(&out);
        } else {
            assert_failure(&out);
        }
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["effective_actor"]["verification"], expected);
        if expected == "verified" {
            assert_eq!(value["effective_actor"]["user"], "server-user");
        } else {
            assert!(value["effective_actor"]["user"].is_null());
        }
        if saved {
            assert_eq!(value["effective_actor"]["stored_user"], "stale-user");
        } else {
            assert!(value["effective_actor"]["stored_user"].is_null());
        }
        assert!(!String::from_utf8_lossy(&out.stdout).contains("secret"));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].headers["authorization"],
            if saved {
                "token saved-secret"
            } else {
                "token env-secret"
            }
        );
    }
}

#[test]
fn offline_status_is_explicitly_unverified_and_never_contacts_the_target() {
    let env = GbTestEnv::new();
    let out = command(&env)
        .env("GB_HOST", "http://127.0.0.1:9")
        .env("GB_TOKEN", "env-secret")
        .env("GB_USER", "not-proof")
        .args(["auth", "status", "--offline", "--json"])
        .output()
        .unwrap();
    assert_success(&out);
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["offline"], true);
    assert_eq!(value["effective_actor"]["verification"], "offline");
    assert!(value["effective_actor"]["user"].is_null());
    assert!(value["effective_actor"]["stored_user"].is_null());
    assert_eq!(value["effective_actor"]["credential_source"], "environment");
}

#[test]
fn scheme_port_deployment_and_alias_ambiguity_bound_credential_selection() {
    let env = GbTestEnv::new();
    fs::write(
        env.path().join("config.toml"),
        r#"
[hosts."https://bucket.test/gitbucket"]
token = "https-secret"
user = "alice"
[hosts."http://bucket.test/gitbucket"]
token = "http-secret"
user = "bob"
protocol = "http"
"#,
    )
    .unwrap();
    for (target, token) in [
        (
            "https://bucket.test/gitbucket/api/v3/",
            Some("https-secret"),
        ),
        ("http://bucket.test/gitbucket", Some("http-secret")),
        ("bucket.test/gitbucket", None),
        ("https://bucket.test:444/gitbucket", None),
        ("https://bucket.test/other", None),
    ] {
        let out = command(&env)
            .args(["auth", "token", "-H", target])
            .output()
            .unwrap();
        if let Some(token) = token {
            assert_success(&out);
            assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), token);
        } else {
            assert_failure(&out);
            assert!(out.stdout.is_empty());
        }
    }
    let out = command(&env)
        .env("GB_PROTOCOL", "http")
        .args(["auth", "token", "-H", "bucket.test/gitbucket"])
        .output()
        .unwrap();
    assert_success(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "http-secret");
}

#[test]
fn migration_preserves_unknown_fields_and_original_bytes_and_is_idempotent() {
    let env = GbTestEnv::new();
    let path = env.path().join("config.toml");
    let original = r#"# original comments and spacing
default_host = "BUCKET.test:443/gitbucket/api/v3/"
future_option = "keep"
[hosts."BUCKET.test:443/gitbucket/api/v3/"]
token = "secret"
user = "alice"
future_host = [1, 2]
[profiles.work]
default_host = "[::1]:8080/gitbucket"
future_profile = true
[profiles.work.hosts."[::1]:8080/gitbucket"]
token = "profile-secret"
user = "bob"
protocol = "http"
"#;
    fs::write(&path, original).unwrap();
    let preview = command(&env).args(["config", "migrate"]).output().unwrap();
    assert_success(&preview);
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    let report: Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(report["changed"], true);
    assert!(report["backup"].is_null());
    let apply = command(&env)
        .args(["config", "migrate", "--apply"])
        .output()
        .unwrap();
    assert_success(&apply);
    assert!(!String::from_utf8_lossy(&apply.stdout).contains("secret"));
    let report: Value = serde_json::from_slice(&apply.stdout).unwrap();
    let backup = report["backup"].as_str().unwrap();
    let migrated: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        migrated["default_host"].as_str(),
        Some("https://bucket.test/gitbucket")
    );
    assert_eq!(
        migrated["hosts"]["https://bucket.test/gitbucket"]["future_host"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        migrated["profiles"]["work"]["default_host"].as_str(),
        Some("http://[::1]:8080/gitbucket")
    );
    assert_eq!(migrated["future_option"].as_str(), Some("keep"));
    assert_eq!(
        migrated["profiles"]["work"]["future_profile"].as_bool(),
        Some(true)
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(backup).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let again = command(&env)
        .args(["config", "migrate", "--apply"])
        .output()
        .unwrap();
    assert_success(&again);
    let again: Value = serde_json::from_slice(&again.stdout).unwrap();
    assert_eq!(again["changed"], false);
    assert!(again["backup"].is_null());
    assert_success(
        &command(&env)
            .args(["config", "migrate", "--restore", backup])
            .output()
            .unwrap(),
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
}

#[test]
fn migration_conflicts_and_restore_after_later_updates_never_overwrite_configuration() {
    let env = GbTestEnv::new();
    let path = env.path().join("config.toml");
    let original = "[hosts.'bucket.test']\ntoken='one'\nuser='a'\n[hosts.'https://bucket.test']\ntoken='two'\nuser='b'\n";
    fs::write(&path, original).unwrap();
    assert_failure(
        &command(&env)
            .args(["auth", "token", "-H", "https://bucket.test"])
            .output()
            .unwrap(),
    );
    assert_failure(
        &command(&env)
            .args(["config", "migrate", "--apply"])
            .output()
            .unwrap(),
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    assert!(!fs::read_dir(env.path()).unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with("config.toml.migration-")));
    fs::write(&path, "[hosts.'bucket.test']\ntoken='one'\nuser='a'\n").unwrap();
    let out = command(&env)
        .args(["config", "migrate", "--apply"])
        .output()
        .unwrap();
    assert_success(&out);
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    fs::write(
        &path,
        "# later edit\n[hosts.'bucket.test']\ntoken='new'\nuser='a'\n",
    )
    .unwrap();
    let before = fs::read(&path).unwrap();
    assert_failure(
        &command(&env)
            .args([
                "config",
                "migrate",
                "--restore",
                report["backup"].as_str().unwrap(),
            ])
            .output()
            .unwrap(),
    );
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn changing_an_explicit_url_scheme_is_rejected_without_saving_partial_fields() {
    let env = GbTestEnv::new();
    let path = env.path().join("config.toml");
    fs::write(&path, SCOPED).unwrap();
    let out = command(&env)
        .args([
            "config",
            "set",
            "host",
            "--host",
            "https://bucket.test/gitbucket",
            "--user",
            "changed",
            "--protocol",
            "http",
        ])
        .output()
        .unwrap();
    assert_failure(&out);
    assert_eq!(fs::read_to_string(&path).unwrap(), SCOPED);
}

#[test]
fn migration_merges_only_identical_aliases_and_keeps_http_and_https_separate() {
    let env = GbTestEnv::new();
    let path = env.path().join("config.toml");
    fs::write(&path, "[hosts.'bucket.test']\ntoken='one'\nuser='a'\n[hosts.'https://BUCKET.test:443/api/v3']\ntoken='one'\nuser='a'\n[hosts.'http://bucket.test']\ntoken='two'\nuser='b'\nprotocol='http'\n").unwrap();
    let out = command(&env)
        .args(["config", "migrate", "--apply"])
        .output()
        .unwrap();
    assert_success(&out);
    let value: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(value["hosts"].as_table().unwrap().len(), 2);
    assert_eq!(
        value["hosts"]["https://bucket.test"]["token"].as_str(),
        Some("one")
    );
    assert_eq!(
        value["hosts"]["http://bucket.test"]["token"].as_str(),
        Some("two")
    );
}

#[cfg(unix)]
#[test]
fn migration_rejects_symlinked_config_and_backup_paths() {
    use std::os::unix::fs::symlink;
    let env = GbTestEnv::new();
    let external = tempfile::tempdir().unwrap();
    let original = "[hosts.'bucket.test']\ntoken='one'\nuser='a'\n";
    let target = external.path().join("original.toml");
    fs::write(&target, original).unwrap();
    let path = env.path().join("config.toml");
    symlink(&target, &path).unwrap();
    assert_failure(
        &command(&env)
            .args(["config", "migrate", "--apply"])
            .output()
            .unwrap(),
    );
    assert_eq!(fs::read_to_string(&target).unwrap(), original);
    fs::remove_file(&path).unwrap();
    fs::write(&path, original).unwrap();
    let out = command(&env)
        .args(["config", "migrate", "--apply"])
        .output()
        .unwrap();
    assert_success(&out);
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    let link = env.path().join("backup-link.json");
    symlink(report["backup"].as_str().unwrap(), &link).unwrap();
    let before = fs::read(&path).unwrap();
    assert_failure(
        &command(&env)
            .args(["config", "migrate", "--restore", link.to_str().unwrap()])
            .output()
            .unwrap(),
    );
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn migration_never_guesses_the_scheme_of_an_uncredentialed_bare_default() {
    let env = GbTestEnv::new();
    let path = env.path().join("config.toml");
    let original = "[profiles.work]\ndefault_host='bucket.test/gitbucket'\n";
    fs::write(&path, original).unwrap();
    let out = command(&env)
        .env("GB_PROTOCOL", "http")
        .args(["config", "migrate", "--apply"])
        .output()
        .unwrap();
    assert_failure(&out);
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
}

#[test]
fn global_logout_reconciles_a_normalized_default_with_a_legacy_storage_key() {
    let env = GbTestEnv::new();
    let path = env.path().join("config.toml");
    fs::write(&path, "default_host='https://bucket.test'\n[hosts.'bucket.test']\ntoken='old'\nuser='a'\n[hosts.'next.test']\ntoken='next'\nuser='b'\n").unwrap();
    assert_success(
        &command(&env)
            .args(["auth", "logout", "--global", "-H", "https://bucket.test"])
            .output()
            .unwrap(),
    );
    let value: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(value["default_host"].as_str(), Some("next.test"));
    let out = command(&env).args(["auth", "token"]).output().unwrap();
    assert_success(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "next");
}

#[test]
fn a_corrupt_backup_never_echoes_its_secret_contents_or_changes_configuration() {
    let env = GbTestEnv::new();
    let path = env.path().join("config.toml");
    fs::write(&path, "[hosts.'bucket.test']\ntoken='one'\nuser='a'\n").unwrap();
    let out = command(&env)
        .args(["config", "migrate", "--apply"])
        .output()
        .unwrap();
    assert_success(&out);
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    let backup_path = report["backup"].as_str().unwrap();
    let mut backup: Value = serde_json::from_slice(&fs::read(backup_path).unwrap()).unwrap();
    backup["original"] = Value::String("token = 'sensitive-unclosed-backup-value".into());
    fs::write(backup_path, serde_json::to_vec(&backup).unwrap()).unwrap();
    let before = fs::read(&path).unwrap();
    let out = command(&env)
        .args(["config", "migrate", "--restore", backup_path])
        .output()
        .unwrap();
    assert_failure(&out);
    assert!(!String::from_utf8_lossy(&out.stderr).contains("sensitive"));
    assert_eq!(fs::read(&path).unwrap(), before);
}
