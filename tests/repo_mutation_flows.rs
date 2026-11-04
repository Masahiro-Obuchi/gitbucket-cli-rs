mod support;

use serde_json::{json, Value};
use support::gb_cmd::{assert_failure, assert_success, stderr, GbTestEnv};
use support::mock_http::{spawn_scripted_server, with_verified_web_session, ScriptedResponse};

fn api(method: &str, path: &str, status: &str, value: Value) -> ScriptedResponse {
    ScriptedResponse::json(
        &format!("{method} /bucket/api/v3/{path} HTTP/1.1"),
        status,
        &value.to_string(),
    )
}
fn repo(name: &str, parent: Option<&str>) -> Value {
    json!({"name":"project","full_name":name,"fork":parent.is_some(),"parent":parent.map(|name|json!({"full_name":name}))})
}
fn preflight(target: &str) -> Vec<ScriptedResponse> {
    vec![
        api("GET", "user", "200 OK", json!({"login":"alice"})),
        api(
            "GET",
            "repos/bob/project",
            "200 OK",
            repo("bob/project", None),
        ),
        api(
            "GET",
            &format!("repos/{target}/project"),
            "404 Not Found",
            json!({}),
        ),
    ]
}

#[test]
fn fork_sends_group_to_rest_and_verifies_owner_and_parent() {
    for group in [None, Some("team")] {
        let env = GbTestEnv::new();
        let target = group.unwrap_or("alice");
        let forked = repo(&format!("{target}/project"), Some("bob/project"));
        let mut script = preflight(target);
        script.extend([
            api(
                "POST",
                "repos/bob/project/forks",
                "201 Created",
                forked.clone(),
            ),
            api("GET", &format!("repos/{target}/project"), "200 OK", forked),
        ]);
        let (port, server) = spawn_scripted_server(script);
        let mut cmd =
            env.repo_api_command(format!("127.0.0.1:{port}/bucket"), "ignored/environment");
        cmd.env("GB_USER", "stale-config-user")
            .env_remove("GB_PASSWORD")
            .args(["repo", "fork", "bob/project"]);
        if let Some(group) = group {
            cmd.args(["--group", group]);
        }
        let output = cmd.output().unwrap();
        assert_success(&output);
        let requests = server.join().unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&requests[3].body).unwrap(),
            group
                .map(|group| json!({"organization":group}))
                .unwrap_or(json!({}))
        );
    }
}

#[test]
fn fork_rejects_wrong_response_or_stored_upstream_without_web_retry() {
    for wrong_response in [true, false] {
        let env = GbTestEnv::new();
        let mut script = preflight("alice");
        script.push(api(
            "POST",
            "repos/bob/project/forks",
            "201 Created",
            repo(
                "alice/project",
                Some(if wrong_response {
                    "wrong/project"
                } else {
                    "bob/project"
                }),
            ),
        ));
        if !wrong_response {
            script.push(api(
                "GET",
                "repos/alice/project",
                "200 OK",
                repo("alice/project", Some("wrong/project")),
            ));
        }
        let (port, server) = spawn_scripted_server(script);
        let output = env
            .api_command(format!("127.0.0.1:{port}/bucket"))
            .args(["--json-errors", "repo", "fork", "bob/project"])
            .output()
            .unwrap();
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["operations"][0]["status"], "unverified");
        server.join().unwrap();
    }
}

#[test]
fn existing_fork_requires_immediate_parent_even_when_network_source_matches() {
    for matching in [true, false] {
        let env = GbTestEnv::new();
        let mut script = preflight("alice");
        script.pop();
        let mut existing = repo(
            "alice/project",
            Some(if matching {
                "bob/project"
            } else {
                "carol/project"
            }),
        );
        existing["source"] = json!({"full_name":"bob/project"});
        script.push(api("GET", "repos/alice/project", "200 OK", existing));
        let (port, server) = spawn_scripted_server(script);
        let output = env
            .api_command(format!("127.0.0.1:{port}/bucket"))
            .args(["repo", "fork", "bob/project"])
            .output()
            .unwrap();
        if matching {
            assert_success(&output);
        } else {
            assert_failure(&output);
        }
        assert_eq!(server.join().unwrap().len(), 3);
    }
}

#[test]
fn fork_rejections_and_server_failure_do_not_start_web_or_retry_writes() {
    for status in [
        "401 Unauthorized",
        "403 Forbidden",
        "409 Conflict",
        "500 Internal Server Error",
    ] {
        let env = GbTestEnv::new();
        let mut script = preflight("alice");
        script.push(api(
            "POST",
            "repos/bob/project/forks",
            status,
            json!({"message":"rejected"}),
        ));
        let (port, server) = spawn_scripted_server(script);
        let output = env
            .api_command(format!("127.0.0.1:{port}/bucket"))
            .args(["--json-errors", "repo", "fork", "bob/project"])
            .output()
            .unwrap();
        assert_failure(&output);
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], "api_error");
        assert_eq!(
            error["error"]["operations"][0]["status"],
            if status.starts_with('5') {
                "unverified"
            } else {
                "failed"
            }
        );
        assert_eq!(server.join().unwrap().len(), 4);
    }
}

#[test]
fn web_fork_reads_back_destination_and_waits_for_async_creation() {
    let env = GbTestEnv::new();
    let mut script = preflight("team");
    script.extend([
        api(
            "POST",
            "repos/bob/project/forks",
            "404 Not Found",
            json!({}),
        ),
        api(
            "GET",
            "repos/bob/project",
            "200 OK",
            repo("bob/project", None),
        ),
        ScriptedResponse::html("POST /bucket/signin HTTP/1.1", "200 OK", "signed in")
            .with_header("set-cookie", "JSESSIONID=web; Path=/"),
        ScriptedResponse::html(
            "POST /bucket/bob/project/fork HTTP/1.1",
            "200 OK",
            "form response",
        ),
        api("GET", "repos/team/project", "404 Not Found", json!({})),
        api(
            "GET",
            "repos/team/project",
            "200 OK",
            repo("team/project", Some("bob/project")),
        ),
    ]);
    let (port, server) = spawn_scripted_server(with_verified_web_session(script));
    let output = env
        .api_command(format!("127.0.0.1:{port}/bucket"))
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "dummy")
        .args(["repo", "fork", "bob/project", "--group", "team"])
        .output()
        .unwrap();
    assert_success(&output);
    let requests = server.join().unwrap();
    let post = requests
        .iter()
        .find(|r| r.target.ends_with("/project/fork"))
        .unwrap();
    assert_eq!(post.body, "account=team");
    assert_eq!(
        post.headers.get("cookie").map(String::as_str),
        Some("JSESSIONID=web")
    );
}

#[test]
fn web_fork_http_200_with_wrong_destination_is_unverified() {
    let env = GbTestEnv::new();
    let mut script = preflight("team");
    script.extend([
        api(
            "POST",
            "repos/bob/project/forks",
            "404 Not Found",
            json!({}),
        ),
        api(
            "GET",
            "repos/bob/project",
            "200 OK",
            repo("bob/project", None),
        ),
        ScriptedResponse::html("POST /bucket/signin HTTP/1.1", "200 OK", "signed in"),
        ScriptedResponse::html("POST /bucket/bob/project/fork HTTP/1.1", "200 OK", "form"),
        api(
            "GET",
            "repos/team/project",
            "200 OK",
            repo("team/project", Some("wrong/project")),
        ),
    ]);
    let (port, server) = spawn_scripted_server(with_verified_web_session(script));
    let output = env
        .api_command(format!("127.0.0.1:{port}/bucket"))
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "dummy")
        .args(["repo", "fork", "bob/project", "--group", "team"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(stderr(&output).contains("repository.fork=unverified"));
    server.join().unwrap();
}

#[test]
fn delete_checks_existence_then_confirms_absence_for_rest_and_web() {
    for web in [false, true] {
        for removed in [false, true] {
            let env = GbTestEnv::new();
            let mut script = vec![
                api(
                    "GET",
                    "repos/alice/project",
                    "200 OK",
                    repo("alice/project", None),
                ),
                api(
                    "DELETE",
                    "repos/alice/project",
                    if web {
                        "404 Not Found"
                    } else {
                        "204 No Content"
                    },
                    json!(null),
                ),
            ];
            if web {
                script.extend([
                    ScriptedResponse::html("POST /bucket/signin HTTP/1.1", "200 OK", "signed in"),
                    ScriptedResponse::html(
                        "POST /bucket/alice/project/settings/delete HTTP/1.1",
                        "200 OK",
                        "form response",
                    ),
                ]);
            }
            script.push(api(
                "GET",
                "repos/alice/project",
                if removed { "404 Not Found" } else { "200 OK" },
                if removed {
                    json!({})
                } else {
                    repo("alice/project", None)
                },
            ));
            let (port, server) = spawn_scripted_server(with_verified_web_session(script));
            let output = env
                .api_command(format!("127.0.0.1:{port}/bucket"))
                .env("GB_USER", "alice")
                .env("GB_PASSWORD", "dummy")
                .args(["repo", "delete", "alice/project", "--yes"])
                .output()
                .unwrap();
            if removed {
                assert_success(&output);
            } else {
                assert_failure(&output);
                assert!(output.stdout.is_empty());
                assert!(stderr(&output).contains("repository.delete=unverified"));
            }
            server.join().unwrap();
        }
    }
}

#[test]
fn deleting_missing_or_private_repository_never_attempts_delete_or_signin() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![api(
        "GET",
        "repos/alice/project",
        "404 Not Found",
        json!({}),
    )]);
    let output = env
        .api_command(format!("127.0.0.1:{port}/bucket"))
        .args(["repo", "delete", "alice/project", "--yes"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert_eq!(server.join().unwrap().len(), 1);
}

#[test]
fn missing_rest_parent_is_verified_from_public_header_without_web_login() {
    for (deployment, prefixed_links) in [
        ("bucket", false),
        ("alice", false),
        ("alice", true),
        ("bob", false),
        ("bob", true),
    ] {
        for existing in [false, true] {
            let env = GbTestEnv::new();
            let mut script = preflight("alice");
            let missing_metadata = json!({"name":"project","full_name":"alice/project"});
            if existing {
                script.pop();
                script.push(api(
                    "GET",
                    "repos/alice/project",
                    "200 OK",
                    missing_metadata,
                ));
            } else {
                script.extend([
                    api(
                        "POST",
                        "repos/bob/project/forks",
                        "201 Created",
                        missing_metadata.clone(),
                    ),
                    api("GET", "repos/alice/project", "200 OK", missing_metadata),
                ]);
            }
            for response in &mut script {
                response.expected_request_line = response
                    .expected_request_line
                    .replace("/bucket/", &format!("/{deployment}/"));
            }
            let page = include_str!("fixtures/gitbucket-4.44-fork-header.html");
            let page = if prefixed_links {
                page.replace("href=\"/", &format!("href=\"/{deployment}/"))
            } else {
                page.to_string()
            };
            script.push(ScriptedResponse::html(
                &format!("GET /{deployment}/alice/project HTTP/1.1"),
                "200 OK",
                &page,
            ));
            let (port, server) = spawn_scripted_server(script);
            let output = env
                .api_command(format!("127.0.0.1:{port}/{deployment}"))
                .env_remove("GB_PASSWORD")
                .args(["repo", "fork", "bob/project"])
                .output()
                .unwrap();
            assert_success(&output);
            assert!(server
                .join()
                .unwrap()
                .iter()
                .all(|r| !r.target.ends_with("/signin")));
        }
    }
}

#[test]
fn existing_private_fork_without_rest_parent_uses_verified_cookie_for_proof_only() {
    let env = GbTestEnv::new();
    let mut script = preflight("alice");
    script.pop();
    script.extend([
        api(
            "GET",
            "repos/alice/project",
            "200 OK",
            json!({"name":"project","full_name":"alice/project","private":true}),
        ),
        ScriptedResponse::html("POST /bucket/signin HTTP/1.1", "200 OK", "signed in")
            .with_header("set-cookie", "JSESSIONID=proof; Path=/"),
        ScriptedResponse::html(
            "GET /bucket/alice/project HTTP/1.1",
            "200 OK",
            include_str!("fixtures/gitbucket-4.44-fork-header.html"),
        ),
    ]);
    let (port, server) = spawn_scripted_server(with_verified_web_session(script));
    let output = env
        .api_command(format!("127.0.0.1:{port}/bucket"))
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "dummy")
        .args(["repo", "fork", "bob/project"])
        .output()
        .unwrap();
    assert_success(&output);
    let requests = server.join().unwrap();
    assert_eq!(
        requests
            .last()
            .unwrap()
            .headers
            .get("cookie")
            .map(String::as_str),
        Some("JSESSIONID=proof")
    );
    assert!(requests
        .iter()
        .all(|r| !r.target.ends_with("/forks") && !r.target.ends_with("/project/fork")));
}
