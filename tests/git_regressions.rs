mod support;

use std::process::Command;
use std::thread;

use tempfile::tempdir;

use support::gb_cmd::gb_command;
use support::git_repo::{git_output, run_git};
use support::mock_http::{spawn_scripted_server, spawn_server, CapturedRequest, ScriptedResponse};

fn serve_json_once(
    expected_request_line: &str,
    expected_auth: &str,
    body: &str,
) -> (u16, thread::JoinHandle<CapturedRequest>) {
    let expected_request_line = expected_request_line.to_string();
    let expected_auth = expected_auth.to_ascii_lowercase();
    let (port, server) = spawn_server("200 OK", body);

    let handle = thread::spawn(move || {
        let request = server.join().unwrap();
        let request_line = format!("{} {} HTTP/1.1", request.method, request.target);
        assert_eq!(request_line, expected_request_line);
        let auth = request
            .headers
            .get("authorization")
            .map(|value| format!("authorization: {}", value).to_ascii_lowercase())
            .unwrap_or_default();
        assert!(auth.contains(&expected_auth));
        request
    });

    (port, handle)
}

#[test]
fn repo_clone_full_url_does_not_require_gb_authentication() {
    let temp = tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    let init = Command::new("git")
        .args(["init", "--bare", remote.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(init.success());

    let clone_url = format!("file://{}", remote.display());
    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args(["repo", "clone", &clone_url, "cloned"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(temp.path().join("cloned").is_dir());
}

#[test]
fn repo_clone_full_url_rejects_missing_profile() {
    let temp = tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    let init = Command::new("git")
        .args(["init", "--bare", remote.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(init.success());
    std::fs::write(temp.path().join("config.toml"), "").unwrap();

    let clone_url = format!("file://{}", remote.display());
    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args([
            "--profile",
            "missing",
            "repo",
            "clone",
            &clone_url,
            "cloned",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Profile 'missing' is not configured"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!temp.path().join("cloned").exists());
}

#[test]
fn pr_merge_returns_non_zero_when_server_reports_not_merged() {
    let temp = tempdir().unwrap();
    let (port, server) = serve_json_once(
        "PUT /api/v3/repos/alice/project/pulls/5/merge HTTP/1.1",
        "authorization: token test-token",
        r#"{"merged":false,"message":"merge conflict"}"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "merge", "5"])
        .output()
        .unwrap();

    server.join().unwrap();

    assert!(
        !output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn repo_list_owner_uses_organization_endpoint_when_owner_is_org() {
    let temp = tempdir().unwrap();
    let (port, server) = serve_json_once(
        "GET /api/v3/orgs/my-org/repos HTTP/1.1",
        "authorization: token test-token",
        "[]",
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["repo", "list", "my-org", "--json"])
        .output()
        .unwrap();

    server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "[]");
}

#[test]
fn auth_logout_accepts_equivalent_hostname_representation() {
    let temp = tempdir().unwrap();
    std::fs::write(
        temp.path().join("config.toml"),
        r#"
default_host = "https://gitbucket.example.com/gitbucket"

[hosts."https://gitbucket.example.com/gitbucket"]
token = "secret-token"
user = "alice"
protocol = "https"
"#,
    )
    .unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args(["auth", "logout", "-H", "gitbucket.example.com/gitbucket"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config = std::fs::read_to_string(temp.path().join("config.toml")).unwrap();
    assert!(
        !config.contains("secret-token"),
        "config was not updated: {config}"
    );
}

#[test]
fn auth_logout_with_profile_removes_fallback_global_credentials() {
    let temp = tempdir().unwrap();
    std::fs::write(
        temp.path().join("config.toml"),
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

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args(["auth", "logout"])
        .output()
        .unwrap();

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

    let config = std::fs::read_to_string(temp.path().join("config.toml")).unwrap();
    assert!(
        !config.contains("global-token"),
        "config was not updated: {config}"
    );
}

#[test]
fn auth_logout_with_profile_prefers_profile_scoped_credentials() {
    let temp = tempdir().unwrap();
    std::fs::write(
        temp.path().join("config.toml"),
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

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args(["auth", "logout"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config = std::fs::read_to_string(temp.path().join("config.toml")).unwrap();
    assert!(
        !config.contains("profile-token"),
        "profile token was not removed: {config}"
    );
    assert!(
        config.contains("global-token"),
        "global token should remain when profile-scoped credentials exist: {config}"
    );
}

#[test]
fn pr_create_fails_cleanly_when_head_is_detached() {
    let temp = tempdir().unwrap();
    run_git(temp.path(), &["init"]);
    run_git(temp.path(), &["config", "user.name", "Test User"]);
    run_git(temp.path(), &["config", "user.email", "test@example.com"]);
    std::fs::write(temp.path().join("README.md"), "hello\n").unwrap();
    run_git(temp.path(), &["add", "README.md"]);
    run_git(temp.path(), &["commit", "-m", "initial"]);
    run_git(temp.path(), &["checkout", "--detach", "HEAD"]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", "gitbucket.example.com")
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .args([
            "pr",
            "create",
            "-t",
            "Detached PR",
            "-b",
            "body",
            "--base",
            "main",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Could not determine current branch"),
        "stderr: {stderr}"
    );
}

#[test]
fn pr_checkout_prefers_matching_remote_when_api_clone_url_is_unusable() {
    let temp = tempdir().unwrap();
    let hosting_root = temp.path().join("hosting");
    let base_bare = hosting_root.join("alice").join("base.git");
    let head_bare = hosting_root.join("bob").join("head.git");
    std::fs::create_dir_all(base_bare.parent().unwrap()).unwrap();
    std::fs::create_dir_all(head_bare.parent().unwrap()).unwrap();
    run_git(
        temp.path(),
        &["init", "--bare", base_bare.to_str().unwrap()],
    );
    run_git(
        temp.path(),
        &["init", "--bare", head_bare.to_str().unwrap()],
    );

    let repos_dir = temp.path().join("repos");
    std::fs::create_dir_all(&repos_dir).unwrap();
    let base_work = repos_dir.join("base-work");
    std::fs::create_dir_all(&base_work).unwrap();
    run_git(&base_work, &["init"]);
    run_git(&base_work, &["config", "user.name", "Test User"]);
    run_git(&base_work, &["config", "user.email", "test@example.com"]);
    std::fs::write(base_work.join("README.md"), "base\n").unwrap();
    run_git(&base_work, &["add", "README.md"]);
    run_git(&base_work, &["commit", "-m", "base"]);
    run_git(&base_work, &["branch", "-M", "main"]);
    run_git(
        &base_work,
        &["remote", "add", "origin", base_bare.to_str().unwrap()],
    );
    run_git(&base_work, &["push", "origin", "main"]);

    let head_work = repos_dir.join("head-work");
    run_git(
        temp.path(),
        &[
            "clone",
            "--branch",
            "main",
            base_bare.to_str().unwrap(),
            head_work.to_str().unwrap(),
        ],
    );
    run_git(&head_work, &["config", "user.name", "Test User"]);
    run_git(&head_work, &["config", "user.email", "test@example.com"]);
    run_git(&head_work, &["checkout", "-b", "feature/demo"]);
    std::fs::write(head_work.join("README.md"), "base\nfeature\n").unwrap();
    run_git(&head_work, &["commit", "-am", "feature"]);
    run_git(
        &head_work,
        &["remote", "add", "fork", head_bare.to_str().unwrap()],
    );
    run_git(&head_work, &["push", "fork", "feature/demo"]);

    let local_repo = temp.path().join("local-repo");
    std::fs::create_dir_all(&local_repo).unwrap();
    run_git(&local_repo, &["init"]);
    run_git(&local_repo, &["config", "user.name", "Test User"]);
    run_git(&local_repo, &["config", "user.email", "test@example.com"]);
    run_git(
        &local_repo,
        &[
            "config",
            &format!("url.file://{}/.insteadOf", hosting_root.display()),
            "https://gitbucket.example.com/",
        ],
    );
    run_git(
        &local_repo,
        &[
            "remote",
            "add",
            "upstream",
            "https://gitbucket.example.com/alice/base.git",
        ],
    );
    run_git(
        &local_repo,
        &[
            "remote",
            "add",
            "fork",
            "https://gitbucket.example.com/bob/head.git",
        ],
    );

    let body = concat!(
        "{",
        "\"number\":5,",
        "\"title\":\"Feature\",",
        "\"state\":\"open\",",
        "\"head\":{\"ref\":\"feature/demo\",\"repo\":{\"name\":\"head\",\"full_name\":\"bob/head\",\"private\":true,\"clone_url\":\"git@gitbucket.example.com:bob/head.git\"}},",
        "\"base\":{\"ref\":\"main\",\"repo\":{\"name\":\"base\",\"full_name\":\"alice/base\",\"private\":false,\"clone_url\":\"git@gitbucket.example.com:alice/base.git\"}}",
        "}"
    )
    .to_string();
    let (port, server) = serve_json_once(
        "GET /api/v3/repos/alice/project/pulls/5 HTTP/1.1",
        "authorization: token test-token",
        &body,
    );

    let output = gb_command()
        .current_dir(&local_repo)
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "checkout", "5"])
        .output()
        .unwrap();

    server.join().unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        git_output(&local_repo, &["branch", "--show-current"]),
        "pr-5"
    );
    let content = std::fs::read_to_string(local_repo.join("README.md")).unwrap();
    assert!(content.contains("feature"), "README content: {content}");
}

#[test]
fn pr_checkout_does_not_overwrite_local_main_when_pr_branch_is_named_main() {
    let temp = tempdir().unwrap();
    let hosting_root = temp.path().join("hosting");
    let base_bare = hosting_root.join("alice").join("base.git");
    let head_bare = hosting_root.join("bob").join("head.git");
    std::fs::create_dir_all(base_bare.parent().unwrap()).unwrap();
    std::fs::create_dir_all(head_bare.parent().unwrap()).unwrap();
    run_git(
        temp.path(),
        &["init", "--bare", base_bare.to_str().unwrap()],
    );
    run_git(
        temp.path(),
        &["init", "--bare", head_bare.to_str().unwrap()],
    );

    let repos_dir = temp.path().join("repos");
    std::fs::create_dir_all(&repos_dir).unwrap();
    let base_work = repos_dir.join("base-work");
    std::fs::create_dir_all(&base_work).unwrap();
    run_git(&base_work, &["init"]);
    run_git(&base_work, &["config", "user.name", "Test User"]);
    run_git(&base_work, &["config", "user.email", "test@example.com"]);
    std::fs::write(base_work.join("README.md"), "base\n").unwrap();
    run_git(&base_work, &["add", "README.md"]);
    run_git(&base_work, &["commit", "-m", "base"]);
    run_git(&base_work, &["branch", "-M", "main"]);
    run_git(
        &base_work,
        &["remote", "add", "origin", base_bare.to_str().unwrap()],
    );
    run_git(&base_work, &["push", "origin", "main"]);

    let head_work = repos_dir.join("head-work");
    run_git(
        temp.path(),
        &[
            "clone",
            "--branch",
            "main",
            base_bare.to_str().unwrap(),
            head_work.to_str().unwrap(),
        ],
    );
    run_git(&head_work, &["config", "user.name", "Test User"]);
    run_git(&head_work, &["config", "user.email", "test@example.com"]);
    run_git(&head_work, &["checkout", "-B", "main"]);
    std::fs::write(head_work.join("README.md"), "base\npr-main\n").unwrap();
    run_git(&head_work, &["commit", "-am", "pr main"]);
    run_git(
        &head_work,
        &["remote", "add", "fork", head_bare.to_str().unwrap()],
    );
    run_git(&head_work, &["push", "fork", "main"]);

    let local_repo = temp.path().join("local-repo");
    std::fs::create_dir_all(&local_repo).unwrap();
    run_git(&local_repo, &["init"]);
    run_git(&local_repo, &["config", "user.name", "Test User"]);
    run_git(&local_repo, &["config", "user.email", "test@example.com"]);
    run_git(
        &local_repo,
        &[
            "config",
            &format!("url.file://{}/.insteadOf", hosting_root.display()),
            "https://gitbucket.example.com/",
        ],
    );
    run_git(
        &local_repo,
        &[
            "remote",
            "add",
            "upstream",
            "https://gitbucket.example.com/alice/base.git",
        ],
    );
    run_git(
        &local_repo,
        &[
            "remote",
            "add",
            "fork",
            "https://gitbucket.example.com/bob/head.git",
        ],
    );
    std::fs::write(local_repo.join("README.md"), "local-main\n").unwrap();
    run_git(&local_repo, &["add", "README.md"]);
    run_git(&local_repo, &["commit", "-m", "local main"]);
    run_git(&local_repo, &["branch", "-M", "main"]);
    let local_main_before = git_output(&local_repo, &["rev-parse", "main"]);

    let body = concat!(
        "{",
        "\"number\":7,",
        "\"title\":\"Main branch PR\",",
        "\"state\":\"open\",",
        "\"head\":{\"ref\":\"main\",\"repo\":{\"name\":\"head\",\"full_name\":\"bob/head\",\"private\":true,\"clone_url\":\"git@gitbucket.example.com:bob/head.git\"}},",
        "\"base\":{\"ref\":\"main\",\"repo\":{\"name\":\"base\",\"full_name\":\"alice/base\",\"private\":false,\"clone_url\":\"git@gitbucket.example.com:alice/base.git\"}}",
        "}"
    );
    let (port, server) = serve_json_once(
        "GET /api/v3/repos/alice/project/pulls/7 HTTP/1.1",
        "authorization: token test-token",
        body,
    );

    let output = gb_command()
        .current_dir(&local_repo)
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "checkout", "7"])
        .output()
        .unwrap();

    server.join().unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        git_output(&local_repo, &["branch", "--show-current"]),
        "pr-7"
    );
    assert_eq!(
        git_output(&local_repo, &["rev-parse", "main"]),
        local_main_before
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Checked out branch 'pr-7'"),
        "stdout: {stdout}"
    );
}
#[test]
fn pr_diff_prefers_matching_remotes_when_api_clone_urls_are_unusable() {
    let temp = tempdir().unwrap();
    let hosting_root = temp.path().join("hosting");
    let base_bare = hosting_root.join("alice").join("base.git");
    let head_bare = hosting_root.join("bob").join("head.git");
    std::fs::create_dir_all(base_bare.parent().unwrap()).unwrap();
    std::fs::create_dir_all(head_bare.parent().unwrap()).unwrap();
    run_git(
        temp.path(),
        &["init", "--bare", base_bare.to_str().unwrap()],
    );
    run_git(
        temp.path(),
        &["init", "--bare", head_bare.to_str().unwrap()],
    );

    let repos_dir = temp.path().join("repos");
    std::fs::create_dir_all(&repos_dir).unwrap();
    let base_work = repos_dir.join("base-work");
    std::fs::create_dir_all(&base_work).unwrap();
    run_git(&base_work, &["init"]);
    run_git(&base_work, &["config", "user.name", "Test User"]);
    run_git(&base_work, &["config", "user.email", "test@example.com"]);
    std::fs::write(base_work.join("README.md"), "base\n").unwrap();
    run_git(&base_work, &["add", "README.md"]);
    run_git(&base_work, &["commit", "-m", "base"]);
    run_git(&base_work, &["branch", "-M", "main"]);
    run_git(
        &base_work,
        &["remote", "add", "origin", base_bare.to_str().unwrap()],
    );
    run_git(&base_work, &["push", "origin", "main"]);

    let head_work = repos_dir.join("head-work");
    run_git(
        temp.path(),
        &[
            "clone",
            "--branch",
            "main",
            base_bare.to_str().unwrap(),
            head_work.to_str().unwrap(),
        ],
    );
    run_git(&head_work, &["config", "user.name", "Test User"]);
    run_git(&head_work, &["config", "user.email", "test@example.com"]);
    run_git(&head_work, &["checkout", "-b", "feature/demo"]);
    std::fs::write(head_work.join("README.md"), "base\nfeature\n").unwrap();
    run_git(&head_work, &["commit", "-am", "feature"]);
    run_git(
        &head_work,
        &["remote", "add", "fork", head_bare.to_str().unwrap()],
    );
    run_git(&head_work, &["push", "fork", "feature/demo"]);

    let local_repo = temp.path().join("local-repo");
    std::fs::create_dir_all(&local_repo).unwrap();
    run_git(&local_repo, &["init"]);
    run_git(&local_repo, &["config", "user.name", "Test User"]);
    run_git(&local_repo, &["config", "user.email", "test@example.com"]);
    run_git(
        &local_repo,
        &[
            "config",
            &format!("url.file://{}/.insteadOf", hosting_root.display()),
            "https://gitbucket.example.com/",
        ],
    );
    run_git(
        &local_repo,
        &[
            "remote",
            "add",
            "upstream",
            "https://gitbucket.example.com/alice/base.git",
        ],
    );
    run_git(
        &local_repo,
        &[
            "remote",
            "add",
            "fork",
            "https://gitbucket.example.com/bob/head.git",
        ],
    );

    let body = concat!(
        "{",
        "\"number\":5,",
        "\"title\":\"Feature\",",
        "\"state\":\"open\",",
        "\"head\":{\"ref\":\"feature/demo\",\"repo\":{\"name\":\"head\",\"full_name\":\"bob/head\",\"private\":true,\"clone_url\":\"git@gitbucket.example.com:bob/head.git\"}},",
        "\"base\":{\"ref\":\"main\",\"repo\":{\"name\":\"base\",\"full_name\":\"alice/base\",\"private\":false,\"clone_url\":\"git@gitbucket.example.com:alice/base.git\"}}",
        "}"
    );
    let (port, server) = serve_json_once(
        "GET /api/v3/repos/alice/project/pulls/5 HTTP/1.1",
        "authorization: token test-token",
        body,
    );

    let output = gb_command()
        .current_dir(&local_repo)
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "diff", "5"])
        .output()
        .unwrap();

    server.join().unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("feature"), "stdout: {stdout}");
}

#[test]
fn pr_diff_returns_non_zero_when_closed_pr_diff_is_unavailable() {
    let temp = tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    run_git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);

    let work = temp.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    run_git(&work, &["init"]);
    run_git(&work, &["config", "user.name", "Test User"]);
    run_git(&work, &["config", "user.email", "test@example.com"]);
    std::fs::write(work.join("README.md"), "base\n").unwrap();
    run_git(&work, &["add", "README.md"]);
    run_git(&work, &["commit", "-m", "base"]);
    run_git(&work, &["branch", "-M", "main"]);
    run_git(
        &work,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    run_git(&work, &["push", "origin", "main"]);

    let local_repo = temp.path().join("local-repo");
    run_git(
        temp.path(),
        &[
            "clone",
            remote.to_str().unwrap(),
            local_repo.to_str().unwrap(),
        ],
    );

    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/9 HTTP/1.1",
            "200 OK",
            "{\"number\":9,\"title\":\"Already merged\",\"state\":\"closed\",\"head\":{\"ref\":\"main\"},\"base\":{\"ref\":\"main\"}}",
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues/9 HTTP/1.1",
            "404 Not Found",
            "{\"message\":\"not found\"}",
        ),
        ScriptedResponse {
            expected_request_line: "GET /alice/project/pull/9.diff HTTP/1.1".into(),
            status_line: "404 Not Found".into(),
            headers: vec![("content-type".into(), "text/plain".into())],
            body: "not found".into(),
        },
    ]);

    let output = gb_command()
        .current_dir(&local_repo)
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "diff", "9", "--no-pager"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 3);
    assert!(
        !output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Diff unavailable"), "stderr: {stderr}");
    assert!(stderr.contains("pull request #9"), "stderr: {stderr}");
}

#[test]
fn pr_diff_uses_saved_diff_when_closed_branch_diff_is_empty() {
    let temp = tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    run_git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);

    let work = temp.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    run_git(&work, &["init"]);
    run_git(&work, &["config", "user.name", "Test User"]);
    run_git(&work, &["config", "user.email", "test@example.com"]);
    std::fs::write(work.join("README.md"), "base\n").unwrap();
    run_git(&work, &["add", "README.md"]);
    run_git(&work, &["commit", "-m", "base"]);
    run_git(&work, &["branch", "-M", "main"]);
    run_git(
        &work,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    run_git(&work, &["push", "origin", "main"]);

    let local_repo = temp.path().join("local-repo");
    run_git(
        temp.path(),
        &[
            "clone",
            remote.to_str().unwrap(),
            local_repo.to_str().unwrap(),
        ],
    );

    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/9 HTTP/1.1",
            "200 OK",
            "{\"number\":9,\"title\":\"Already merged\",\"state\":\"closed\",\"diff_url\":\"/alice/project/pull/9.diff\",\"head\":{\"ref\":\"main\"},\"base\":{\"ref\":\"main\"}}",
        ),
        ScriptedResponse {
            expected_request_line: "GET /alice/project/pull/9.diff HTTP/1.1".into(),
            status_line: "200 OK".into(),
            headers: vec![("content-type".into(), "text/plain".into())],
            body: "diff --git a/README.md b/README.md\n+saved\n".into(),
        },
    ]);

    let output = gb_command()
        .current_dir(&local_repo)
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "diff", "9", "--no-pager"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 2);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("+saved"), "stdout: {stdout}");
}

#[test]
fn pr_diff_rejects_non_diff_saved_diff_response() {
    let temp = tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    run_git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);

    let work = temp.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    run_git(&work, &["init"]);
    run_git(&work, &["config", "user.name", "Test User"]);
    run_git(&work, &["config", "user.email", "test@example.com"]);
    std::fs::write(work.join("README.md"), "base\n").unwrap();
    run_git(&work, &["add", "README.md"]);
    run_git(&work, &["commit", "-m", "base"]);
    run_git(&work, &["branch", "-M", "main"]);
    run_git(
        &work,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    run_git(&work, &["push", "origin", "main"]);

    let local_repo = temp.path().join("local-repo");
    run_git(
        temp.path(),
        &[
            "clone",
            remote.to_str().unwrap(),
            local_repo.to_str().unwrap(),
        ],
    );

    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/9 HTTP/1.1",
            "200 OK",
            "{\"number\":9,\"title\":\"Already merged\",\"state\":\"closed\",\"diff_url\":\"/alice/project/pull/9.diff\",\"head\":{\"ref\":\"main\"},\"base\":{\"ref\":\"main\"}}",
        ),
        ScriptedResponse {
            expected_request_line: "GET /alice/project/pull/9.diff HTTP/1.1".into(),
            status_line: "200 OK".into(),
            headers: vec![("content-type".into(), "text/html".into())],
            body: "<html><body>Please sign in</body></html>".into(),
        },
        ScriptedResponse {
            expected_request_line: "GET /alice/project/pull/9.diff HTTP/1.1".into(),
            status_line: "404 Not Found".into(),
            headers: vec![("content-type".into(), "text/plain".into())],
            body: "not found".into(),
        },
    ]);

    let output = gb_command()
        .current_dir(&local_repo)
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "diff", "9", "--no-pager"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 3);
    assert!(
        !output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("response was not a diff"),
        "stderr: {stderr}"
    );
}
