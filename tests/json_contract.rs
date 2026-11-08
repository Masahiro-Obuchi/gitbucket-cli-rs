mod support;

use serde_json::{json, Value};
use support::gb_cmd::{assert_failure, assert_success, GbTestEnv};
use support::mock_http::{spawn_scripted_server, ScriptedResponse};

const API: &str = "/bucket/api/v3";
const REPO: &str = "/bucket/api/v3/repos/alice/project";

fn response(path: &str, value: &Value) -> ScriptedResponse {
    ScriptedResponse::json(
        &format!("GET {path} HTTP/1.1"),
        "200 OK",
        &value.to_string(),
    )
}
fn run(script: Vec<ScriptedResponse>, args: &[&str]) -> std::process::Output {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(script);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .args(args)
        .output()
        .unwrap();
    server.join().unwrap();
    output
}
fn value(output: &std::process::Output) -> Value {
    assert_success(output);
    serde_json::from_slice(&output.stdout).unwrap()
}
fn issue() -> Value {
    json!({"number":7,"title":"日本語 Issue","state":"open"})
}
fn pr() -> Value {
    json!({"number":7,"title":"Draft PR","state":"open"})
}
fn details() -> Value {
    json!([{"id":51,"body":"本文","future_comment":true}])
}

#[test]
fn issue_comments_keep_count_and_match_the_pinned_json_shape() {
    let expected: Value =
        serde_json::from_str(include_str!("fixtures/json/issue-with-comments.json")).unwrap();
    let mut api = expected.clone();
    api.as_object_mut().unwrap().remove("comment_details");
    let output = run(
        vec![
            response(&format!("{REPO}/issues/7"), &api),
            response(
                &format!("{REPO}/issues/7/comments?per_page=100"),
                &details(),
            ),
        ],
        &["issue", "view", "7", "--comments", "--json"],
    );
    assert_eq!(value(&output), expected);
    // No comments request or output augmentation without the flag.
    assert_eq!(
        value(&run(
            vec![response(&format!("{REPO}/issues/7"), &api)],
            &["issue", "view", "7", "--json"]
        )),
        api
    );
    for count in [None, Some(Value::Null), Some(json!(0))] {
        let mut api = issue();
        if let Some(count) = count {
            api["comments"] = count;
        }
        let mut expected = api.clone();
        expected["comment_details"] = json!([]);
        let output = run(
            vec![
                response(&format!("{REPO}/issues/7"), &api),
                response(
                    &format!("{REPO}/issues/7/comments?per_page=100"),
                    &json!([]),
                ),
            ],
            &["issue", "view", "7", "--comments", "--json"],
        );
        assert_eq!(value(&output), expected);
    }
}

#[test]
fn pr_comments_keep_the_legacy_array_and_preserve_api_metadata() {
    let expected: Value =
        serde_json::from_str(include_str!("fixtures/json/pr-with-comments.json")).unwrap();
    let mut api = expected.clone();
    api.as_object_mut().unwrap().remove("api_comments");
    api["comments"] = json!(12);
    let output = run(
        vec![
            response(&format!("{REPO}/pulls/7"), &api),
            response(
                &format!("{REPO}/issues/7/comments?per_page=100"),
                &details(),
            ),
        ],
        &["pr", "view", "7", "--comments", "--json"],
    );
    assert_eq!(value(&output), expected);
    assert_eq!(
        value(&run(
            vec![response(&format!("{REPO}/pulls/7"), &api)],
            &["pr", "view", "7", "--json"]
        )),
        api
    );
    let output = run(
        vec![
            response(&format!("{REPO}/pulls/7"), &pr()),
            response(
                &format!("{REPO}/issues/7/comments?per_page=100"),
                &details(),
            ),
        ],
        &["pr", "comment", "list", "7", "--json"],
    );
    assert_eq!(value(&output), details());
}

#[test]
fn unknown_nested_fields_and_missing_null_false_values_survive_typed_reads() {
    for optional in [
        None,
        Some(Value::Null),
        Some(json!(false)),
        Some(json!(true)),
    ] {
        let mut api = pr();
        api["future"] = json!({"list":[false,null,{},[],"🌸\n本文"],"nested":{"unknown":"kept"}});
        api["head"] = json!({"ref":"topic","unknown_head":17,"repo":{"name":"project","full_name":"alice/project","owner":{"login":"alice","unknown_user":true},"unknown_repo":[1,2]}});
        api["labels"] = json!([{"name":"日本語 label","unknown_label":{"value":null}}]);
        api["assignees"] = json!([{"login":"bob","unknown_user":false}]);
        if let Some(optional) = optional {
            api["draft"] = optional.clone();
            api["mergeable"] = optional;
        }
        let output = run(
            vec![response(&format!("{REPO}/pulls/7"), &api)],
            &["pr", "view", "7", "--json"],
        );
        assert_eq!(value(&output), api);
    }
    // Response types also back repository/label/milestone lists; defaults such
    // as private=false must not become invented API facts in their JSON.
    for (path, args, api) in [
        (
            format!("{API}/user/repos"),
            vec!["repo", "list", "--json"],
            json!({"name":"project","full_name":"alice/project","future":null}),
        ),
        (
            format!("{REPO}/labels"),
            vec!["label", "list", "--json"],
            json!({"name":"bug","future":false}),
        ),
        (
            format!("{REPO}/milestones?state=open"),
            vec!["milestone", "list", "--json"],
            json!({"number":1,"title":"release","state":"open","future":{"a":[]}}),
        ),
    ] {
        let expected = json!([api]);
        assert_eq!(
            value(&run(vec![response(&path, &expected)], &args)),
            expected
        );
    }
}

#[test]
fn pr_list_uses_canonical_fields_but_retains_unknown_collection_metadata() {
    let mut summary = pr();
    summary["head"] = json!({"ref":"topic","repo":{"name":"project","full_name":"alice/project"}});
    summary["body"] = json!("summary-only body");
    summary["draft"] = json!(false);
    summary["collection_extension"] = json!({"future":null});
    summary["future"] = json!("summary");
    let mut canonical = pr();
    canonical["future"] = json!("detail wins");
    let mut expected = canonical.clone();
    expected["collection_extension"] = summary["collection_extension"].clone();
    let output = run(
        vec![
            response(&format!("{REPO}/pulls?state=all"), &json!([summary])),
            response(&format!("{REPO}/pulls/7"), &canonical),
        ],
        &["pr", "list", "--state", "all", "--json"],
    );
    assert_eq!(value(&output), json!([expected]));
}

#[test]
fn issue_json_comments_enumerate_links_before_printing_and_propagate_failures() {
    for fail in [false, true] {
        let api = issue();
        let last = if fail {
            ScriptedResponse::json(
                &format!("GET {REPO}/issues/7/comments?page=2 HTTP/1.1"),
                "503 Service Unavailable",
                "{}",
            )
        } else {
            response(
                &format!("{REPO}/issues/7/comments?page=2"),
                &json!([{"id":52,"body":"second page","future":null}]),
            )
        };
        let output = run(
            vec![
                response(&format!("{REPO}/issues/7"), &api),
                response(
                    &format!("{REPO}/issues/7/comments?per_page=100"),
                    &details(),
                )
                .with_header("link", "<?page=2>; rel=next"),
                last,
            ],
            &[
                "--json-errors",
                "issue",
                "view",
                "7",
                "--comments",
                "--json",
            ],
        );
        if fail {
            assert_failure(&output);
            assert!(output.stdout.is_empty());
        } else {
            let out = value(&output);
            assert_eq!(out["comment_details"].as_array().unwrap().len(), 2);
            assert!(out.get("comments").is_none());
            assert!(out["comment_details"][1]["future"].is_null());
        }
    }
}

#[test]
fn reserved_augmentation_fields_are_never_silently_overwritten() {
    for kind in ["issue", "pr"] {
        let mut api = if kind == "issue" { issue() } else { pr() };
        let route = if kind == "issue" { "issues" } else { "pulls" };
        if kind == "issue" {
            api["comment_details"] = json!({"server":"data"});
        } else {
            api["comments"] = json!(12);
            api["api_comments"] = json!({"server":"data"});
        }
        let output = run(
            vec![
                response(&format!("{REPO}/{route}/7"), &api),
                response(
                    &format!("{REPO}/issues/7/comments?per_page=100"),
                    &details(),
                ),
            ],
            &["--json-errors", kind, "view", "7", "--comments", "--json"],
        );
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("without losing server data"));
    }
}

#[test]
fn pr_text_shows_reported_metadata_and_distinguishes_unknown_draft() {
    let mut api = pr();
    api["draft"] = json!(true);
    api["labels"] = json!([{"name":"review"}]);
    api["assignees"] = json!([{"login":"bob"}]);
    api["merged_by"] = json!({"login":"alice"});
    let output = run(
        vec![response(&format!("{REPO}/pulls/7"), &api)],
        &["pr", "view", "7", "--no-pager"],
    );
    assert_success(&output);
    let text = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "DRAFT",
        "Draft: yes",
        "Labels: review",
        "Assignees: bob",
        "Merged by: alice",
    ] {
        assert!(text.contains(expected), "{text}");
    }
    let output = run(
        vec![response(&format!("{REPO}/pulls/7"), &pr())],
        &["pr", "view", "7", "--no-pager"],
    );
    assert_success(&output);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Draft:"));
}

#[test]
fn unknown_fields_survive_create_wrappers_and_comment_mutation_responses() {
    for comment in [false, true] {
        let api = if comment {
            json!({"id":51,"body":"created","future":{"kept":true},"user":{"login":"alice","future_user":null}})
        } else {
            let mut api = pr();
            api["draft"] = json!(false);
            api["future"] = json!({"kept":true});
            api
        };
        let mut script = Vec::new();
        if comment {
            script.push(response(&format!("{REPO}/pulls/7"), &pr()));
        }
        let path = if comment {
            format!("{REPO}/issues/7/comments")
        } else {
            format!("{REPO}/pulls")
        };
        let body = if comment {
            api.to_string()
        } else {
            json!(api.to_string()).to_string()
        };
        script.push(ScriptedResponse::json(
            &format!("POST {path} HTTP/1.1"),
            "200 OK",
            &body,
        ));
        let args = if comment {
            vec!["pr", "comment", "7", "--body", "created", "--json"]
        } else {
            vec![
                "pr", "create", "--head", "topic", "--base", "main", "--title", "created",
                "--body", "body", "--json",
            ]
        };
        assert_eq!(value(&run(script, &args)), api);
    }
}

#[test]
fn paged_json_lists_keep_each_pages_metadata_and_known_type_validation() {
    let mut newer = issue();
    newer["future"] = json!({"page":1});
    let mut older = issue();
    older["number"] = json!(6);
    older["future"] = json!({"page":2});
    let output = run(
        vec![
            response(&format!("{REPO}/issues?state=all"), &json!([newer.clone()]))
                .with_header("link", "<?page=2>; rel=next"),
            response(
                &format!("{REPO}/issues?page=2&state=all"),
                &json!([older.clone()]),
            ),
        ],
        &["issue", "list", "--state", "all", "--json"],
    );
    assert_eq!(value(&output), json!([newer, older]));
    let mut invalid = pr();
    invalid["number"] = json!(null);
    let output = run(
        vec![response(&format!("{REPO}/pulls/7"), &invalid)],
        &["--json-errors", "pr", "view", "7", "--json"],
    );
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stderr).unwrap()["error"]["code"],
        "json_error"
    );
}

#[test]
fn unknown_json_numbers_keep_large_integer_and_decimal_precision() {
    // Construct the HTTP body directly: json! or Value would already round
    // these numbers before the CLI receives them if precision is disabled.
    let body = r#"{"number":7,"title":"precise metadata","state":"open","future":{"large_id":18446744073709551617,"decimal":0.12345678901234567890123456789}}"#;
    for list in [false, true] {
        let script = if list {
            vec![
                ScriptedResponse::json(
                    &format!("GET {REPO}/pulls?state=open HTTP/1.1"),
                    "200 OK",
                    &format!("[{body}]"),
                ),
                response(&format!("{REPO}/pulls/7"), &pr()),
            ]
        } else {
            vec![ScriptedResponse::json(
                &format!("GET {REPO}/pulls/7 HTTP/1.1"),
                "200 OK",
                body,
            )]
        };
        let args = if list {
            vec!["pr", "list", "--json"]
        } else {
            vec!["pr", "view", "7", "--json"]
        };
        let actual = value(&run(script, &args));
        let actual = if list { &actual[0] } else { &actual };
        assert_eq!(
            actual["future"]["large_id"].to_string(),
            "18446744073709551617"
        );
        assert_eq!(
            actual["future"]["decimal"].to_string(),
            "0.12345678901234567890123456789"
        );
    }
}

#[test]
fn preserving_unknown_numbers_does_not_relax_known_id_validation() {
    let max = u64::MAX.to_string();
    let output = run(
        vec![response(
            &format!("{REPO}/pulls/{max}"),
            &json!({"number":u64::MAX,"title":"largest identity","state":"open"}),
        )],
        &["pr", "view", &max, "--json"],
    );
    assert_eq!(value(&output)["number"].as_u64(), Some(u64::MAX));
    for number in ["18446744073709551616", "-1", "7.5"] {
        let body = format!(r#"{{"number":{number},"title":"invalid identity","state":"open"}}"#);
        let output = run(
            vec![ScriptedResponse::json(
                &format!("GET {REPO}/pulls/7 HTTP/1.1"),
                "200 OK",
                &body,
            )],
            &["--json-errors", "pr", "view", "7", "--json"],
        );
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stderr).unwrap()["error"]["code"],
            "json_error"
        );
    }
}
