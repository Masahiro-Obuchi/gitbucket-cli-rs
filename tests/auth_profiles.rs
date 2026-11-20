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
fn explicit_default_host_does_not_lose_its_scheme_when_using_global_fallback() {
    let env = GbTestEnv::new();
    let path = env.path().join("config.toml");
    fs::write(&path, "[hosts.'http://bucket.test']\ntoken='global-http'\nuser='global'\n[profiles.work.hosts.'https://bucket.test']\ntoken='profile-https'\nuser='profile'\n").unwrap();
    assert_success(
        &command(&env)
            .args([
                "config",
                "set",
                "default-host",
                "http://bucket.test",
                "--profile",
                "work",
            ])
            .output()
            .unwrap(),
    );
    let out = command(&env)
        .args(["auth", "status", "--offline", "--json", "--profile", "work"])
        .output()
        .unwrap();
    assert_success(&out);
    let status: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(status["effective_actor"]["instance"], "http://bucket.test");
    assert_eq!(status["effective_actor"]["credential_source"], "global");
    let config: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        config["profiles"]["work"]["default_host"].as_str(),
        Some("http://bucket.test")
    );
    assert_eq!(
        config["hosts"]["http://bucket.test"]["token"].as_str(),
        Some("global-http")
    );
}

#[test]
fn profile_logout_does_not_change_a_default_to_another_scheme() {
    let env = GbTestEnv::new();
    let path = env.path().join("config.toml");
    fs::write(&path, "[hosts.'https://bucket.test']\ntoken='global-https'\nuser='global'\n[profiles.work]\ndefault_host='http://bucket.test'\n[profiles.work.hosts.'http://bucket.test']\ntoken='profile-http'\nuser='profile'\n").unwrap();
    assert_success(
        &command(&env)
            .args(["auth", "logout", "--profile", "work"])
            .output()
            .unwrap(),
    );
    let out = command(&env)
        .args(["auth", "status", "--offline", "--json", "--profile", "work"])
        .output()
        .unwrap();
    assert_success(&out);
    let status: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(status["effective_actor"].is_null(), "{status}");
    let config: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        config["profiles"]["work"]["default_host"].as_str(),
        Some("http://bucket.test")
    );
    assert_eq!(
        config["hosts"]["https://bucket.test"]["token"].as_str(),
        Some("global-https")
    );
}

#[test]
fn global_logout_clears_default_without_selecting_another_host() {
    let env = GbTestEnv::new();
    let path = env.path().join("config.toml");
    fs::write(&path, "default_host='https://old.test'\n[hosts.'https://old.test']\ntoken='old'\nuser='old'\n[hosts.'http://bucket.test']\ntoken='http-token'\nuser='a'\n").unwrap();
    assert_success(&command(&env).args(["auth", "logout"]).output().unwrap());

    let value: toml::Value = toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    assert!(value.get("default_host").is_none());
    assert!(value["hosts"].get("https://old.test").is_none());
    assert_eq!(
        value["hosts"]["http://bucket.test"]["token"].as_str(),
        Some("http-token")
    );

    let out = command(&env).args(["auth", "token"]).output().unwrap();
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
            fs::write(env.path().join("config.toml"), format!("default_host = '{target}'\n[hosts.'{target}']\ntoken = 'saved-secret'\nuser = 'stale-user'\n")).unwrap();
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
        assert_eq!(value.as_object().unwrap().len(), 3);
        assert!(value.get("hosts").is_none());
        assert!(value.get("profiles").is_none());
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
fn scheme_port_deployment_and_input_ambiguity_bound_credential_selection() {
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
fn config_rejects_legacy_and_noncanonical_instances_without_rewriting() {
    let env = GbTestEnv::new();
    let path = env.path().join("config.toml");
    for original in [
        "[hosts.'bucket.test']\ntoken='hidden'\nuser='alice'\n",
        "[profiles.work.hosts.'https://BUCKET.test:443/gitbucket/api/v3/']\ntoken='hidden'\nuser='alice'\n",
        "[profiles.work]\ndefault_host='bucket.test/gitbucket'\n",
    ] {
        fs::write(&path, original).unwrap();
        let output = command(&env).args(["auth", "status", "--offline"]).output().unwrap();
        assert_failure(&output);
        let diagnostics = String::from_utf8_lossy(&output.stderr);
        assert!(diagnostics.contains("Saved GitBucket instances must use normalized HTTP(S) base URLs"));
        assert!(!diagnostics.contains("hidden"));
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
    }
}
