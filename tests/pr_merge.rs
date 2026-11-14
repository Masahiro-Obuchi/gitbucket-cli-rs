mod support;

use serde_json::{json, Value};
use support::gb_cmd::{assert_failure, assert_success, GbTestEnv};
use support::mock_http::{spawn_scripted_server, ScriptedResponse};

const HEAD: &str = "1111111111111111111111111111111111111111";
const OTHER: &str = "2222222222222222222222222222222222222222";
const MERGE: &str = "3333333333333333333333333333333333333333";
fn pr() -> Value {
    json!({"number":5,"title":"PR","state":"open","merged":false,"draft":false,"mergeable":null,"head":{"ref":"topic","sha":HEAD},"base":{"ref":"main","repo":{"name":"project","full_name":"alice/project"}}})
}
fn get(pr: &Value) -> ScriptedResponse {
    ScriptedResponse::json(
        "GET /bucket/api/v3/repos/alice/project/pulls/5 HTTP/1.1",
        "200 OK",
        &pr.to_string(),
    )
}
fn put(status: &str, value: Value) -> ScriptedResponse {
    ScriptedResponse::json(
        "PUT /bucket/api/v3/repos/alice/project/pulls/5/merge HTTP/1.1",
        status,
        &value.to_string(),
    )
}
fn run(
    responses: Vec<ScriptedResponse>,
    args: &[&str],
) -> (
    std::process::Output,
    Vec<support::mock_http::CapturedRequest>,
) {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(responses);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .args(["--json-errors", "pr", "merge", "5"])
        .args(args)
        .output()
        .unwrap();
    // Assert before joining so an input-validation regression does not wait on an unused script.
    (output, server.join().unwrap())
}
fn error(output: &std::process::Output) -> Value {
    assert_failure(output);
    assert!(output.stdout.is_empty());
    serde_json::from_slice::<Value>(&output.stderr).unwrap()["error"].clone()
}

#[test]
fn preflight_refuses_draft_closed_missing_or_invalid_heads_before_merge() {
    for (field, value, cause) in [
        ("draft", json!(true), "draft"),
        ("state", json!("closed"), "not_open"),
        ("merged", json!(true), "not_open"),
        ("mergeable", json!(false), "not_mergeable"),
        ("head", Value::Null, "missing_head"),
        ("head", json!({"ref":"topic","sha":"HEAD"}), "missing_head"),
        ("number", json!(6), "target_mismatch"),
        (
            "base",
            json!({"ref":"main","repo":{"name":"other","full_name":"alice/other"}}),
            "target_mismatch",
        ),
    ] {
        let mut before = pr();
        before[field] = value;
        let (output, requests) = run(vec![get(&before)], &[]);
        let error = error(&output);
        assert_eq!(error["code"], "merge_check_failed", "{error}");
        assert_eq!(error["cause"], cause);
        assert_eq!(error["operations"][0]["status"], "not_attempted");
        assert!(error.get("outcome").is_none());
        assert_eq!(requests.len(), 1);
    }
}

#[test]
fn match_head_commit_mismatch_and_changes_on_fresh_read_never_merge() {
    let env = GbTestEnv::new();
    for sha in ["main", "abcd", "0000000000000000000000000000000000000000"] {
        let output = env
            .command()
            .args([
                "--json-errors",
                "pr",
                "merge",
                "5",
                "--match-head-commit",
                sha,
            ])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
    }
    let (output, requests) = run(vec![get(&pr())], &["--match-head-commit", OTHER]);
    assert_eq!(error(&output)["cause"], "head_mismatch");
    assert_eq!(requests.len(), 1);
    for field in ["sha", "draft", "state", "merged", "mergeable"] {
        let mut fresh = pr();
        match field {
            "sha" => fresh["head"]["sha"] = json!(OTHER),
            "state" => fresh[field] = json!("closed"),
            "mergeable" => fresh[field] = json!(false),
            _ => fresh[field] = json!(true),
        }
        let (output, requests) = run(vec![get(&pr()), get(&fresh)], &[]);
        let error = error(&output);
        assert_eq!(error["operations"][0]["status"], "not_attempted");
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|r| r.method == "GET"));
    }
}

#[test]
fn success_requires_response_and_canonical_readback_and_exposes_nonatomic_checks() {
    for explicit in [false, true] {
        let mut after = pr();
        after["state"] = json!("closed");
        after["merged"] = json!(true);
        let identity = if explicit {
            format!("AnyObjectId[{MERGE}]")
        } else {
            MERGE.to_owned()
        };
        let result =
            json!({"sha":identity,"merged":true,"message":"merged","unknown":{"kept":true}});
        let mut args = vec!["--message", "Ship it", "--json"];
        if explicit {
            args.extend(["--match-head-commit", HEAD]);
        }
        let (output, requests) = run(
            vec![
                get(&pr()),
                get(&pr()),
                put("200 OK", result.clone()),
                get(&after),
            ],
            &args,
        );
        assert_success(&output);
        assert!(output.stderr.is_empty());
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["head_commit"], HEAD);
        assert_eq!(report["atomic"], false);
        assert_eq!(report["verification"], "client_preflight_and_readback");
        assert_eq!(report["result"], result);
        let payload: Value = serde_json::from_str(&requests[2].body).unwrap();
        assert_eq!(payload, json!({"commit_message":"Ship it"}));
        assert!(requests.iter().all(|r| !r.target.contains("comments")));
    }
}

#[test]
fn rejection_failure_and_unconfirmed_responses_are_not_retried_or_replaced_by_comments() {
    for status in [
        "401 Unauthorized",
        "403 Forbidden",
        "405 Method Not Allowed",
        "409 Conflict",
        "500 Internal Server Error",
    ] {
        let (output, requests) = run(
            vec![
                get(&pr()),
                get(&pr()),
                put(status, json!({"message":"denied"})),
            ],
            &[],
        );
        let error = error(&output);
        assert_eq!(error["status"], status[..3].parse::<u16>().unwrap());
        assert_eq!(
            error["operations"][0]["status"],
            if status.starts_with('5') {
                "unverified"
            } else {
                "failed"
            }
        );
        assert_eq!(requests.len(), 3);
    }
    for result in [
        json!({"merged":false}),
        json!({"merged":true}),
        json!({"merged":true,"sha":""}),
    ] {
        let (output, requests) = run(vec![get(&pr()), get(&pr()), put("200 OK", result)], &[]);
        assert_eq!(error(&output)["outcome"], "unverified");
        assert_eq!(requests.len(), 3);
    }
}

#[test]
fn failed_or_changed_readback_remains_unverified_after_a_successful_response() {
    for mode in ["open", "head", "number", "null", "http"] {
        let mut after = pr();
        after["merged"] = json!(true);
        after["state"] = json!("closed");
        match mode {
            "open" => after["merged"] = json!(false),
            "head" => after["head"]["sha"] = json!(OTHER),
            "number" => after["number"] = json!(6),
            "null" => after = Value::Null,
            _ => {}
        }
        let readback = if mode == "http" {
            ScriptedResponse::json(
                "GET /bucket/api/v3/repos/alice/project/pulls/5 HTTP/1.1",
                "503 Service Unavailable",
                r#"{"message":"readback unavailable"}"#,
            )
        } else {
            get(&after)
        };
        let (output, requests) = run(
            vec![
                get(&pr()),
                get(&pr()),
                put("200 OK", json!({"merged":true,"sha":MERGE})),
                readback,
            ],
            &[],
        );
        let error = error(&output);
        assert_eq!(error["outcome"], "unverified");
        assert_eq!(error["operations"][0]["status"], "unverified");
        assert_eq!(requests.len(), 4);
    }
}
