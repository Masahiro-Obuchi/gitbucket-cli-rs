mod support;

use serde_json::{json, Value};
use support::gb_cmd::{assert_failure, assert_success, stderr, GbTestEnv};
use support::mock_http::{spawn_scripted_server, ScriptedResponse};

const ISSUES: &str = "/bucket/api/v3/repos/alice/project/issues";
const PULLS: &str = "/bucket/api/v3/repos/alice/project/pulls";
const COMMENTS: &str = "/bucket/api/v3/repos/alice/project/issues/7/comments";

fn response(path: &str, value: Value) -> ScriptedResponse {
    ScriptedResponse::json(
        &format!("GET {path} HTTP/1.1"),
        "200 OK",
        &value.to_string(),
    )
}
fn item(id: u64) -> Value {
    json!({"number":id,"title":format!("Item {id}"),"state":"open"})
}
fn pr(id: u64, branch: &str) -> Value {
    json!({"number":id,"title":format!("PR {id}"),"state":"open","head":{"ref":branch,"repo":{"name":"project","full_name":"alice/project"}},"base":{"ref":"main","repo":{"name":"project","full_name":"alice/project"}}})
}
fn comment(id: u64) -> Value {
    json!({"id":id,"body":format!("comment-{id}"),"user":{"login":"alice"}})
}
fn error(output: &std::process::Output) -> Value {
    assert_failure(output);
    assert!(output.stdout.is_empty());
    serde_json::from_slice::<Value>(&output.stderr).unwrap()["error"].clone()
}

#[test]
fn fixed_issue_and_pr_pages_are_complete_at_25_item_boundaries() {
    for count in [0, 1, 25, 26, 50, 51] {
        for command in ["issue", "pr"] {
            let env = GbTestEnv::new();
            let path = if command == "issue" { ISSUES } else { PULLS };
            let mut responses = Vec::new();
            let mut start = 1;
            loop {
                let page: Vec<_> = (start..(start + 25).min(count + 1)).map(item).collect();
                let query = if responses.is_empty() {
                    format!("{path}?state=all")
                } else {
                    format!("{path}?state=all&page={}", responses.len() + 1)
                };
                let length = page.len();
                responses.push(response(&query, json!(page)));
                if length < 25 {
                    break;
                }
                start += 25;
            }
            let pages = responses.len();
            if command == "pr" {
                responses.extend(
                    (1..=count)
                        .rev()
                        .map(|id| response(&format!("{PULLS}/{id}"), item(id))),
                );
            }
            let requests = pages + if command == "pr" { count as usize } else { 0 };
            let (port, server) = spawn_scripted_server(responses);
            let output = env
                .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
                .args([command, "list", "--state", "all", "--json"])
                .output()
                .unwrap();
            assert_success(&output);
            let items: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(items.as_array().unwrap().len(), count as usize);
            assert_eq!(
                items
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v["number"].as_u64().unwrap())
                    .collect::<Vec<_>>(),
                (1..=count).rev().collect::<Vec<_>>()
            );
            assert_eq!(server.join().unwrap().len(), requests);
        }
    }
}

#[test]
fn comments_without_links_are_all_rows_even_when_count_matches_a_page_boundary() {
    for count in [0, 1, 25, 26, 50, 51, 100, 101] {
        let env = GbTestEnv::new();
        let (port, server) = spawn_scripted_server(vec![
            response(&format!("{PULLS}/7"), item(7)),
            response(
                &format!("{COMMENTS}?per_page=100"),
                json!((1..=count).rev().map(comment).collect::<Vec<_>>()),
            ),
        ]);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .args(["pr", "comment", "list", "7", "--json"])
            .output()
            .unwrap();
        assert_success(&output);
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value.as_array().unwrap().len(), count as usize);
        assert_eq!(
            value
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["id"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            (1..=count).collect::<Vec<_>>()
        );
        assert_eq!(server.join().unwrap().len(), 2);
    }
}

#[test]
fn query_path_and_root_relative_links_resolve_against_the_response_and_keep_state() {
    for (link, target) in [
        ("?page=2", format!("{ISSUES}?page=2&state=closed")),
        ("issues?page=2", format!("{ISSUES}?page=2&state=closed")),
        (
            "../project/issues?page=2",
            format!("{ISSUES}?page=2&state=closed"),
        ),
        (
            "/bucket/api/v3/repos/alice/project/issues?state=closed&page=2",
            format!("{ISSUES}?state=closed&page=2"),
        ),
    ] {
        let env = GbTestEnv::new();
        let (port, server) = spawn_scripted_server(vec![
            response(&format!("{ISSUES}?state=closed"), json!([item(3), item(2)])).with_header(
                "link",
                &format!("<{link}>; rel=\"prev next\"; title=\"comma, semicolon; title\""),
            ),
            response(&target, json!([item(1)])),
        ]);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .args(["issue", "list", "--state", "closed", "--json"])
            .output()
            .unwrap();
        assert_success(&output);
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout)
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(server.join().unwrap().len(), 2);
    }
}

#[test]
fn link_pages_deduplicate_ids_and_preserve_the_first_observation() {
    let env = GbTestEnv::new();
    let mut changed = item(2);
    changed["title"] = json!("changed later");
    let (port, server) = spawn_scripted_server(vec![
        response(
            &format!("{ISSUES}?state=all"),
            json!([item(2), item(3), item(2)]),
        )
        .with_header("link", "<?page=2>; rel=next"),
        response(
            &format!("{ISSUES}?page=2&state=all"),
            json!([item(1), changed]),
        ),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .args(["issue", "list", "--state", "all", "--json"])
        .output()
        .unwrap();
    assert_success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value.as_array().unwrap().len(), 3);
    assert_eq!(value[1]["title"], "Item 2");
    server.join().unwrap();
}

#[test]
fn unrelated_link_relations_do_not_end_fixed_page_traversal() {
    for relation in ["canonical", "self", "describedby"] {
        let env = GbTestEnv::new();
        let (port, server) = spawn_scripted_server(vec![
            response(
                &format!("{ISSUES}?state=all"),
                json!((2..=26).map(item).collect::<Vec<_>>()),
            )
            .with_header(
                "link",
                &format!("<https://docs.example.test/schema>; rel={relation}"),
            ),
            response(&format!("{ISSUES}?state=all&page=2"), json!([item(1)])),
        ]);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .args(["issue", "list", "--state", "all", "--json"])
            .output()
            .unwrap();
        assert_success(&output);
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            value.as_array().unwrap().len(),
            26,
            "unrelated rel={relation} truncated the collection"
        );
        assert_eq!(server.join().unwrap().len(), 2);
    }
}

#[test]
fn a_terminal_page_without_any_new_ids_is_not_a_complete_result() {
    for linked in [false, true] {
        let env = GbTestEnv::new();
        let mut first = response(
            &format!("{ISSUES}?state=open"),
            json!((1..=25).map(item).collect::<Vec<_>>()),
        );
        let second = if linked {
            first = first.with_header("link", "<?page=2>; rel=next");
            response(
                &format!("{ISSUES}?page=2&state=open"),
                json!((1..=25).map(item).collect::<Vec<_>>()),
            )
            .with_header("link", "<?page=1>; rel=prev")
        } else {
            response(
                &format!("{ISSUES}?state=open&page=2"),
                json!((1..=10).map(item).collect::<Vec<_>>()),
            )
        };
        let (port, server) = spawn_scripted_server(vec![first, second]);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .args(["--json-errors", "issue", "list", "--json"])
            .output()
            .unwrap();
        let e = error(&output);
        assert_eq!(e["cause"], "pagination_no_progress");
        assert_eq!(server.join().unwrap().len(), 2);
    }
}

#[cfg(unix)]
#[test]
fn cancelling_a_recheck_preserves_the_uncertain_create_response() {
    use std::io::Write;
    use std::net::TcpListener;
    use std::process::Stdio;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};
    use support::mock_http::{accept_with_timeout, read_request, write_response};

    for json_errors in [false, true] {
        let env = GbTestEnv::new();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let server = thread::spawn(move || {
            let mut requests = Vec::new();
            let mut first = accept_with_timeout(&listener);
            requests.push(read_request(&mut first));
            assert_eq!(requests[0].target, format!("{PULLS}?state=open"));
            write_response(
                &mut first,
                "200 OK",
                &[("content-type".into(), "application/json".into())],
                "[]",
            );
            drop(first);
            let mut post = accept_with_timeout(&listener);
            requests.push(read_request(&mut post));
            assert_eq!(requests[1].method, "POST");
            // The server may already have created the PR. Its response body is
            // incomplete, so the source error must remain unverified.
            post.write_all(b"HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{").unwrap();
            drop(post);
            let mut recheck = accept_with_timeout(&listener);
            requests.push(read_request(&mut recheck));
            assert_eq!(requests[2].method, "GET");
            ready_tx.send(()).unwrap();
            let _ = done_rx.recv_timeout(Duration::from_secs(8));
            drop(recheck);
            requests
        });
        let mut command = env.repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project");
        if json_errors {
            command.arg("--json-errors");
        }
        let mut child = command
            .args([
                "pr",
                "create",
                "--head",
                "topic",
                "--base",
                "main",
                "--title",
                "New",
                "--body",
                "Body",
                "--detect-existing",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        if ready_rx.recv_timeout(Duration::from_secs(8)).is_err() {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("existing-PR recheck did not start");
        }
        unsafe {
            assert_eq!(libc::kill(child.id() as i32, libc::SIGTERM), 0);
        }
        let started = Instant::now();
        while child.try_wait().unwrap().is_none() && started.elapsed() < Duration::from_secs(5) {
            thread::sleep(Duration::from_millis(10));
        }
        if child.try_wait().unwrap().is_none() {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("recheck did not cancel");
        }
        let output = child.wait_with_output().unwrap();
        let _ = done_tx.send(());
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 3);
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        if json_errors {
            let e = error(&output);
            assert_eq!(e["code"], "http_error");
            assert_eq!(e["cause"], "body_read_failed");
            assert_eq!(e["status"], 201);
            assert_eq!(e["outcome"], "unverified");
        } else {
            assert!(
                stderr(&output).contains("Outcome is unverified"),
                "{}",
                stderr(&output)
            );
        }
        assert!(stderr(&output).contains("cancelled"));
    }
}

#[test]
fn cyclic_or_reordered_equivalent_links_fail_before_another_request() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        response(&format!("{ISSUES}?state=all"), json!([item(3)]))
            .with_header("link", "<?page=2>; rel=next"),
        response(&format!("{ISSUES}?page=2&state=all"), json!([item(2)]))
            .with_header("link", "<?state=all&%70age=2>; rel=next"),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .args(["--json-errors", "issue", "list", "--state", "all", "--json"])
        .output()
        .unwrap();
    let e = error(&output);
    assert_eq!(e["code"], "pagination_incomplete");
    assert_eq!(e["cause"], "pagination_cycle");
    assert_eq!(server.join().unwrap().len(), 2);
}

#[test]
fn ignored_pages_unknown_size_and_page_cap_never_return_partial_success() {
    for mode in ["ignored", "unknown-size", "cap"] {
        let env = GbTestEnv::new();
        let first = if mode == "unknown-size" {
            26
        } else if mode == "ignored" {
            25
        } else {
            1
        };
        let mut responses = vec![response(
            &format!("{ISSUES}?state=open"),
            json!((1..=first).map(item).collect::<Vec<_>>()),
        )];
        if mode == "ignored" {
            responses.push(response(
                &format!("{ISSUES}?state=open&page=2"),
                json!((1..=25).map(item).collect::<Vec<_>>()),
            ));
        }
        if mode == "cap" {
            responses[0] = response(&format!("{ISSUES}?state=open"), json!([item(1)]))
                .with_header("link", "<?page=2>; rel=next");
            responses.push(
                response(&format!("{ISSUES}?page=2&state=open"), json!([item(2)]))
                    .with_header("link", "<?page=3>; rel=next"),
            );
        }
        let requests = responses.len();
        let (port, server) = spawn_scripted_server(responses);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .env("GB_PAGINATION_MAX_PAGES", "2")
            .args(["--json-errors", "issue", "list", "--json"])
            .output()
            .unwrap();
        let e = error(&output);
        assert_eq!(e["code"], "pagination_incomplete");
        assert_eq!(
            e["cause"],
            match mode {
                "ignored" => "pagination_no_progress",
                "unknown-size" => "unknown_pagination",
                _ => "page_limit",
            }
        );
        assert_eq!(server.join().unwrap().len(), requests);
    }
}

#[test]
fn unsafe_or_malformed_links_and_changed_filters_are_rejected_without_echoing_urls() {
    for link in [
        "<https://foreign.invalid/api/v3/repos/alice/project/issues?token=private-value>; rel=next",
        "<//foreign.invalid/path>; rel=next",
        "</api/v3/repos/alice/project/issues>; rel=next",
        "</bucket/api/v3/repos/alice/other/issues>; rel=next",
        "</bucket/api/v3/%252e%252e/outside>; rel=next",
        "<?state=closed&page=2>; rel=next",
        "<?page=2#fragment>; rel=next",
        "garbage; rel=last",
        "<?page=2>; rel=\"next",
        "<?page=2>; rel=next; rel=prev",
        "<?page=2>; rel=next, <?page=3>; rel=next",
    ] {
        let env = GbTestEnv::new();
        let (port, server) = spawn_scripted_server(vec![response(
            &format!("{ISSUES}?state=open"),
            json!([item(1)]),
        )
        .with_header("link", link)]);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .args(["--json-errors", "issue", "list", "--json"])
            .output()
            .unwrap();
        assert_eq!(error(&output)["code"], "pagination_incomplete");
        assert!(!stderr(&output).contains("private-value"));
        assert_eq!(server.join().unwrap().len(), 1);
    }
}

#[test]
fn detect_existing_and_null_detail_fallback_search_the_second_fixed_page() {
    for lookup in [false, true] {
        let env = GbTestEnv::new();
        let mut responses = Vec::new();
        let state = if lookup { "all" } else { "open" };
        if lookup {
            responses.push(response(&format!("{PULLS}/1"), Value::Null));
        }
        responses.push(response(
            &format!("{PULLS}?state={state}"),
            json!((2..=26).map(|n| pr(n, "unrelated")).collect::<Vec<_>>()),
        ));
        responses.push(response(
            &format!("{PULLS}?state={state}&page=2"),
            json!([pr(1, "topic")]),
        ));
        if !lookup {
            responses.push(response(&format!("{PULLS}/1"), pr(1, "topic")));
        }
        let (port, server) = spawn_scripted_server(responses);
        let mut command = env.repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project");
        if lookup {
            command.args(["pr", "view", "1", "--json"]);
        } else {
            command.args([
                "pr",
                "create",
                "--head",
                "topic",
                "--base",
                "main",
                "--detect-existing",
            ]);
        }
        let output = command.output().unwrap();
        assert_success(&output);
        if lookup {
            assert!(serde_json::from_slice::<Value>(&output.stdout).unwrap()["head"].is_null());
            assert_eq!(
                serde_json::from_slice::<Value>(&output.stdout).unwrap()["number"],
                1
            );
        } else {
            assert!(
                String::from_utf8_lossy(&output.stdout).contains("Found existing pull request #1")
            );
        }
        assert!(server.join().unwrap().iter().all(|r| r.method == "GET"));
    }
}

#[test]
fn failed_second_page_or_incomparable_detail_blocks_pr_creation() {
    for mode in [
        "page",
        "detail",
        "incomparable",
        "conflicting",
        "wrong-number",
    ] {
        let env = GbTestEnv::new();
        let mut responses = Vec::new();
        if mode == "page" {
            responses.push(response(
                &format!("{PULLS}?state=open"),
                json!((1..=25).map(|n| pr(n, "different")).collect::<Vec<_>>()),
            ));
            responses.push(ScriptedResponse::json(
                &format!("GET {PULLS}?state=open&page=2 HTTP/1.1"),
                "500 Internal Server Error",
                "{}",
            ));
        } else {
            let candidate = json!({"number":7,"title":"unknown","state":"open","head":{"ref":"topic"},"base":{"ref":"main"}});
            responses.push(response(
                &format!("{PULLS}?state=open"),
                json!([candidate.clone()]),
            ));
            if mode == "detail" {
                responses.push(ScriptedResponse::json(
                    &format!("GET {PULLS}/7 HTTP/1.1"),
                    "404 Not Found",
                    "{}",
                ));
            } else {
                let mut actual = if mode == "incomparable" {
                    candidate
                } else {
                    pr(7, "topic")
                };
                if mode == "conflicting" {
                    actual["head"]["label"] = json!("bob:topic");
                }
                if mode == "wrong-number" {
                    actual["number"] = json!(8);
                }
                responses.push(response(&format!("{PULLS}/7"), actual));
            }
        }
        let (port, server) = spawn_scripted_server(responses);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .args([
                "--json-errors",
                "pr",
                "create",
                "--title",
                "New",
                "--body",
                "body",
                "--head",
                "topic",
                "--base",
                "main",
                "--detect-existing",
            ])
            .output()
            .unwrap();
        error(&output);
        assert!(server.join().unwrap().iter().all(|r| r.method == "GET"));
    }
}

#[test]
fn view_comment_list_and_edit_last_share_linked_comment_enumeration() {
    for mode in ["view", "list", "edit-last"] {
        let env = GbTestEnv::new();
        let mut responses = vec![response(&format!("{PULLS}/7"), item(7))];
        if mode == "edit-last" {
            responses.push(response("/bucket/api/v3/user", json!({"login":"alice"})));
        }
        responses.push(
            response(&format!("{COMMENTS}?per_page=100"), json!([comment(1)]))
                .with_header("link", "<?page=2&per_page=100>; rel=next"),
        );
        responses.push(response(
            &format!("{COMMENTS}?page=2&per_page=100"),
            json!([comment(51)]),
        ));
        if mode == "edit-last" {
            responses.push(ScriptedResponse::json(
                "PATCH /bucket/api/v3/repos/alice/project/issues/comments/51 HTTP/1.1",
                "200 OK",
                &comment(51).to_string(),
            ));
        }
        let (port, server) = spawn_scripted_server(responses);
        let mut command = env.repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project");
        match mode {
            "view" => command.args(["pr", "view", "7", "--comments", "--json"]),
            "list" => command.args(["pr", "comment", "list", "7", "--json"]),
            _ => command.args(["pr", "comment", "7", "--edit-last", "--body", "changed"]),
        };
        let output = command.output().unwrap();
        assert_success(&output);
        if mode != "edit-last" {
            let value: Value = serde_json::from_slice(&output.stdout).unwrap();
            let items = if mode == "view" {
                &value["comments"]
            } else {
                &value
            };
            assert_eq!(items.as_array().unwrap().len(), 2);
            assert_eq!(items[1]["id"], 51);
        }
        let requests = server.join().unwrap();
        assert!(requests
            .iter()
            .filter(|r| r.method == "PATCH")
            .all(|r| r.target.ends_with("/51")));
    }
}

#[test]
fn absolute_links_and_multiple_header_fields_are_supported_but_credentials_are_rejected() {
    for credentials in [false, true] {
        let env = GbTestEnv::new();
        let (port, server) = support::mock_http::spawn_scripted_server_with(|port| {
            let auth = if credentials {
                "alice:private-value@"
            } else {
                ""
            };
            let mut responses = vec![response(&format!("{ISSUES}?state=open"), json!([item(2)]))
                .with_header("link", "<?page=1>; rel=prev")
                .with_header(
                    "link",
                    &format!("<http://{auth}127.0.0.1:{port}{ISSUES}?page=2>; rel=next"),
                )];
            if !credentials {
                responses.push(response(
                    &format!("{ISSUES}?page=2&state=open"),
                    json!((3..=27).map(item).collect::<Vec<_>>()),
                ));
            }
            responses
        });
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .args(["--json-errors", "issue", "list", "--json"])
            .output()
            .unwrap();
        if credentials {
            assert_eq!(error(&output)["cause"], "unsafe_next_link");
            assert!(!stderr(&output).contains("private-value"));
        } else {
            // Last Link page has 25 rows: no speculative numeric page follows.
            assert_success(&output);
            assert_eq!(
                serde_json::from_slice::<Value>(&output.stdout)
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .len(),
                26
            );
        }
        assert_eq!(
            server.join().unwrap().len(),
            if credentials { 1 } else { 2 }
        );
    }
}

#[test]
fn null_detail_does_not_return_an_early_match_if_a_later_page_fails() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        response(&format!("{PULLS}/1"), Value::Null),
        response(
            &format!("{PULLS}?state=all"),
            json!((1..=25).map(item).collect::<Vec<_>>()),
        ),
        ScriptedResponse::json(
            &format!("GET {PULLS}?state=all&page=2 HTTP/1.1"),
            "503 Service Unavailable",
            "{}",
        ),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .args(["--json-errors", "pr", "view", "1", "--json"])
        .output()
        .unwrap();
    assert_eq!(error(&output)["status"], 503);
    assert_eq!(server.join().unwrap().len(), 3);
}

#[test]
fn failed_create_rechecks_all_pages_without_retrying_the_post() {
    for failed_page in [false, true] {
        let env = GbTestEnv::new();
        let mut responses = vec![
            response(&format!("{PULLS}?state=open"), json!([])),
            ScriptedResponse::json(
                &format!("POST {PULLS} HTTP/1.1"),
                "500 Internal Server Error",
                "{\"message\":\"create uncertain\"}",
            ),
            response(
                &format!("{PULLS}?state=open"),
                json!((2..=26).map(|n| pr(n, "other")).collect::<Vec<_>>()),
            ),
        ];
        if failed_page {
            responses.push(ScriptedResponse::json(
                &format!("GET {PULLS}?state=open&page=2 HTTP/1.1"),
                "503 Service Unavailable",
                "{\"message\":\"page unavailable\"}",
            ));
        } else {
            responses.push(response(
                &format!("{PULLS}?state=open&page=2"),
                json!([pr(1, "topic")]),
            ));
            responses.push(response(&format!("{PULLS}/1"), pr(1, "topic")));
        }
        let (port, server) = spawn_scripted_server(responses);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .args([
                "--json-errors",
                "pr",
                "create",
                "--title",
                "New",
                "--body",
                "body",
                "--head",
                "topic",
                "--base",
                "main",
                "--detect-existing",
                "--json",
            ])
            .output()
            .unwrap();
        if failed_page {
            let error = error(&output);
            assert_eq!(error["status"], 500);
            assert!(error["message"]
                .as_str()
                .unwrap()
                .contains("page unavailable"));
        } else {
            assert_success(&output);
            assert_eq!(
                serde_json::from_slice::<Value>(&output.stdout).unwrap()["number"],
                1
            );
        }
        assert_eq!(
            server
                .join()
                .unwrap()
                .iter()
                .filter(|r| r.method == "POST")
                .count(),
            1
        );
    }
}

#[test]
fn an_unverifiable_recheck_never_claims_the_prior_create_did_not_happen() {
    for missing_detail in [false, true] {
        let env = GbTestEnv::new();
        let detail = if missing_detail {
            Value::Null
        } else {
            let mut detail = pr(7, "topic");
            detail["head"]["label"] = json!("other-owner:topic");
            detail
        };
        let (port, server) = spawn_scripted_server(vec![
            response(&format!("{PULLS}?state=open"), json!([])),
            ScriptedResponse::json(
                &format!("POST {PULLS} HTTP/1.1"),
                "500 Internal Server Error",
                r#"{"message":"create outcome unknown"}"#,
            ),
            response(&format!("{PULLS}?state=open"), json!([pr(7, "topic")])),
            response(&format!("{PULLS}/7"), detail),
        ]);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .args([
                "--json-errors",
                "pr",
                "create",
                "--title",
                "New",
                "--body",
                "Body",
                "--head",
                "topic",
                "--base",
                "main",
                "--detect-existing",
            ])
            .output()
            .unwrap();
        let e = error(&output);
        assert_eq!(e["status"], 500);
        assert_eq!(e["outcome"], "unverified");
        assert!(
            !e["message"]
                .as_str()
                .unwrap()
                .to_ascii_lowercase()
                .contains("no pr was created"),
            "{e}"
        );
        assert_eq!(
            server
                .join()
                .unwrap()
                .iter()
                .filter(|r| r.method == "POST")
                .count(),
            1
        );
    }
}

#[test]
fn invalid_page_bounds_fail_before_the_first_request() {
    let env = GbTestEnv::new();
    for limit in ["0", "100001", "-1", "abc", ""] {
        let output = env
            .repo_api_command("127.0.0.1:1/bucket", "alice/project")
            .env("GB_PAGINATION_MAX_PAGES", limit)
            .args(["--json-errors", "issue", "list", "--json"])
            .output()
            .unwrap();
        assert!(error(&output)["message"]
            .as_str()
            .unwrap()
            .contains("GB_PAGINATION_MAX_PAGES"));
    }
}

#[test]
fn json_lists_require_canonical_details_but_tables_use_the_complete_summary() {
    for mode in ["null", "wrong-number", "http", "table"] {
        let env = GbTestEnv::new();
        let mut responses = vec![response(
            &format!("{PULLS}?state=closed"),
            json!([pr(7, "topic")]),
        )];
        if mode != "table" {
            responses.push(match mode {
                "http" => ScriptedResponse::json(
                    &format!("GET {PULLS}/7 HTTP/1.1"),
                    "404 Not Found",
                    "{}",
                ),
                "null" => response(&format!("{PULLS}/7"), Value::Null),
                _ => response(&format!("{PULLS}/7"), pr(8, "topic")),
            });
        }
        let (port, server) = spawn_scripted_server(responses);
        let mut command = env.repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project");
        command.args([
            "--json-errors",
            "pr",
            "list",
            "--state",
            "closed",
            "--no-pager",
        ]);
        if mode != "table" {
            command.arg("--json");
        }
        let output = command.output().unwrap();
        if mode == "table" {
            assert_success(&output);
        } else {
            error(&output);
        }
        assert_eq!(
            server.join().unwrap().len(),
            if mode == "table" { 1 } else { 2 }
        );
    }
}
