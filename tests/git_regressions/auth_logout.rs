use crate::support::gb_cmd::GbTestEnv;

#[test]
fn auth_logout_accepts_equivalent_hostname_representation() {
    let env = GbTestEnv::new();
    std::fs::write(
        env.path().join("config.toml"),
        r#"
default_host = "https://gitbucket.example.com/gitbucket"

[hosts."https://gitbucket.example.com/gitbucket"]
token = "secret-token"
user = "alice"
protocol = "https"
"#,
    )
    .unwrap();

    let output = env
        .command()
        .args(["auth", "logout", "-H", "gitbucket.example.com/gitbucket"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config = std::fs::read_to_string(env.path().join("config.toml")).unwrap();
    assert!(
        !config.contains("secret-token"),
        "config was not updated: {config}"
    );
}

#[test]
fn auth_logout_with_profile_removes_fallback_global_credentials() {
    let env = GbTestEnv::new();
    std::fs::write(
        env.path().join("config.toml"),
        r#"
default_profile = "work"

[profiles.work]
default_host = "gitbucket.example.com"
default_repo = "alice/project"

[hosts."gitbucket.example.com"]
token = "global-token"
user = "alice"
protocol = "https"
"#,
    )
    .unwrap();

    let output = env.command().args(["auth", "logout"]).output().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("global credentials used by profile work"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let config = std::fs::read_to_string(env.path().join("config.toml")).unwrap();
    assert!(
        !config.contains("global-token"),
        "config was not updated: {config}"
    );
}

#[test]
fn auth_logout_with_profile_prefers_profile_scoped_credentials() {
    let env = GbTestEnv::new();
    std::fs::write(
        env.path().join("config.toml"),
        r#"
default_profile = "work"

[profiles.work]
default_host = "gitbucket.example.com"

[profiles.work.hosts."gitbucket.example.com"]
token = "profile-token"
user = "alice"
protocol = "https"

[hosts."gitbucket.example.com"]
token = "global-token"
user = "bob"
protocol = "https"
"#,
    )
    .unwrap();

    let output = env.command().args(["auth", "logout"]).output().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config = std::fs::read_to_string(env.path().join("config.toml")).unwrap();
    assert!(
        !config.contains("profile-token"),
        "profile token was not removed: {config}"
    );
    assert!(
        config.contains("global-token"),
        "global token should remain when profile-scoped credentials exist: {config}"
    );
}
