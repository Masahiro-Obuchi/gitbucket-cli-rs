mod support;

use std::net::TcpListener;
use std::process::Output;

use support::gb_cmd::{assert_failure, assert_success, stderr, GbTestEnv};
use support::git_repo::{init_work_repo, run_git};
use support::mock_http::spawn_server;

fn origin(env: &GbTestEnv, url: &str) {
    init_work_repo(env.path());
    run_git(env.path(), &["remote", "add", "origin", url]);
}

fn no_request(listener: &TcpListener, output: &Output) {
    assert_failure(output);
    listener.set_nonblocking(true).unwrap();
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn implicit_repository_from_other_origin_is_rejected_before_a_write() {
    for command in [
        vec!["issue", "create", "-t", "Title", "-b", "Body"],
        vec!["repo", "fork"],
    ] {
        let env = GbTestEnv::new();
        origin(&env, "https://other.example/alice/demo.git");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let output = env
            .api_command(format!(
                "127.0.0.1:{}",
                listener.local_addr().unwrap().port()
            ))
            .env_remove("GB_REPO")
            .args(command)
            .output()
            .unwrap();
        no_request(&listener, &output);
        assert!(stderr(&output).contains("Git origin does not unambiguously match"));
    }
}

#[test]
fn implicit_repository_from_other_deployment_is_rejected_before_a_write() {
    let env = GbTestEnv::new();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    origin(
        &env,
        &format!("http://127.0.0.1:{port}/other/git/alice/demo.git"),
    );
    let output = env
        .api_command(format!("127.0.0.1:{port}/gitbucket"))
        .env_remove("GB_REPO")
        .args(["issue", "close", "7"])
        .output()
        .unwrap();
    no_request(&listener, &output);
    assert!(stderr(&output).contains("--repo OWNER/REPO"));
}

#[test]
fn configured_default_host_is_not_silently_combined_with_another_remote() {
    let env = GbTestEnv::new();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    origin(&env, "git@other.example:alice/demo.git");
    std::fs::write(
        env.path().join("config.toml"),
        format!(
            r#"
default_host = "http://127.0.0.1:{port}"
[hosts."http://127.0.0.1:{port}"]
token = "test-token"
user = "alice"
protocol = "http"
"#
        ),
    )
    .unwrap();
    let output = env
        .command()
        .env_remove("GB_HOST")
        .env_remove("GB_REPO")
        .args(["issue", "list"])
        .output()
        .unwrap();
    no_request(&listener, &output);
}

#[test]
fn matching_http_and_ssh_origins_resolve_the_same_instance_and_repository() {
    for (base, ssh) in [
        ("", false),
        ("/gitbucket", false),
        ("", true),
        ("/gitbucket", true),
    ] {
        let env = GbTestEnv::new();
        let (port, server) = spawn_server("200 OK", "[]");
        let remote = if ssh {
            format!("ssh://git@127.0.0.1:29418{base}/alice/demo.git")
        } else {
            format!("http://127.0.0.1:{port}{base}/git/alice/demo.git")
        };
        origin(&env, &remote);
        let output = env
            .api_command(format!("127.0.0.1:{port}{base}"))
            .env_remove("GB_REPO")
            .args(["issue", "list", "--json"])
            .output()
            .unwrap();
        let request = server.join().unwrap();
        assert_success(&output);
        assert_eq!(
            request.target,
            format!("{base}/api/v3/repos/alice/demo/issues?state=open")
        );
    }
}

#[test]
fn explicit_and_profile_repositories_do_not_depend_on_origin() {
    for profile in [false, true] {
        let env = GbTestEnv::new();
        origin(&env, "git@other.example:wrong/repo.git");
        let (port, server) = spawn_server("200 OK", "[]");
        let mut command = env.api_command(format!("127.0.0.1:{port}"));
        command.env_remove("GB_REPO");
        if profile {
            std::fs::write(
                env.path().join("config.toml"),
                "[profiles.work]\ndefault_repo = 'alice/demo'\n",
            )
            .unwrap();
            command.args(["--profile", "work"]);
        } else {
            command.args(["-R", "alice/demo"]);
        }
        let output = command.args(["issue", "list", "--json"]).output().unwrap();
        let request = server.join().unwrap();
        assert_success(&output);
        assert_eq!(request.target, "/api/v3/repos/alice/demo/issues?state=open");
    }
}

#[test]
fn malformed_repository_names_are_rejected_before_network_access() {
    for repo in [
        "a/b/c", "../r", "a/..", "a/r?x", "a/r#x", "a/%2e%2e", "a/r\\x",
    ] {
        let env = GbTestEnv::new();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let output = env
            .repo_api_command(
                format!("127.0.0.1:{}", listener.local_addr().unwrap().port()),
                repo,
            )
            .args(["issue", "create", "-t", "Title", "-b", "Body"])
            .output()
            .unwrap();
        no_request(&listener, &output);
        assert!(stderr(&output).contains("Invalid repository format"));
    }
}

#[test]
fn relative_api_escapes_are_rejected_before_network_access() {
    for endpoint in [
        "../../outside",
        "/%2e%2e/%2e%2e/outside",
        "/%252e%252e%252f%252e%252e/outside",
        "/%3f/%252e%252e/%252e%252e/%252e%252e/outside",
        "user#fragment",
    ] {
        let env = GbTestEnv::new();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let output = env
            .api_command(format!(
                "127.0.0.1:{}/gitbucket",
                listener.local_addr().unwrap().port()
            ))
            .args(["api", endpoint, "--input", "-"])
            .output()
            .unwrap();
        no_request(&listener, &output);
        assert!(stderr(&output).contains("configured GitBucket API base"));
    }
}

#[test]
fn redirects_cannot_move_authenticated_api_requests_outside_the_base() {
    use support::mock_http::{spawn_scripted_server, ScriptedResponse};
    for same_origin in [false, true] {
        let foreign = TcpListener::bind("127.0.0.1:0").unwrap();
        let destination = format!(
            "http://127.0.0.1:{}/outside",
            foreign.local_addr().unwrap().port()
        );
        let destination = if same_origin {
            "/outside".to_string()
        } else {
            destination
        };
        let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
            "GET /gitbucket/api/v3/user HTTP/1.1",
            "302 Found",
            "",
        )
        .with_header("location", &destination)]);
        let env = GbTestEnv::new();
        let output = env
            .api_command(format!("127.0.0.1:{port}/gitbucket"))
            .args(["api", "user"])
            .output()
            .unwrap();
        server.join().unwrap();
        no_request(&foreign, &output);
        assert!(stderr(&output).contains("API error (302)"));
    }
}

#[test]
fn owner_arguments_cannot_inject_repository_api_paths() {
    for args in [
        vec!["repo", "list", "../other"],
        vec!["repo", "create", "demo", "--group", "team?query"],
    ] {
        let env = GbTestEnv::new();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let output = env
            .api_command(format!(
                "127.0.0.1:{}",
                listener.local_addr().unwrap().port()
            ))
            .args(args)
            .output()
            .unwrap();
        no_request(&listener, &output);
        assert!(stderr(&output).contains("Invalid repository format"));
    }
}
