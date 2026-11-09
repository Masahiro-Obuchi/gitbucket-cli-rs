mod support;
use serde_json::{json, Value};
use support::{
    gb_cmd::{assert_failure, assert_success, GbTestEnv},
    mock_http::{spawn_scripted_server, with_verified_web_session, ScriptedResponse},
};
const PR: &str = "/bucket/api/v3/repos/alice/project/pulls/1";
const WEB: &str = "/bucket/alice/project/pull/1";
const PAGE: &str = include_str!("fixtures/gitbucket-4.44-review-conversation.html");
fn pr(sha: &str) -> Value {
    json!({"number":1,"title":"PR","state":"open","head":{"ref":"topic","sha":sha},"base":{"ref":"main"}})
}
fn get(path: &str, value: &Value) -> ScriptedResponse {
    ScriptedResponse::json(
        &format!("GET {path} HTTP/1.1"),
        "200 OK",
        &value.to_string(),
    )
}
fn script(mode: &str) -> Vec<ScriptedResponse> {
    let mut raw = pr(&"a".repeat(40));
    if mode == "collision" {
        raw["review_comments"] = json!("server metadata");
    }
    let mut frames = vec![get(PR, if mode == "empty" { &Value::Null } else { &raw })];
    if mode == "empty" {
        frames[0] = ScriptedResponse::json(&format!("GET {PR} HTTP/1.1"), "200 OK", "");
        frames.push(get(
            "/bucket/api/v3/repos/alice/project/pulls?state=all",
            &json!([raw.clone()]),
        ));
    }
    if mode == "public-http" {
        frames.push(ScriptedResponse::html(
            &format!("GET {WEB} HTTP/1.1"),
            "500 Internal Server Error",
            "{}",
        ));
        return frames;
    }
    if !mode.starts_with("public") {
        frames.push(ScriptedResponse::html(
            &format!("GET {WEB} HTTP/1.1"),
            "401 Unauthorized",
            "",
        ));
        frames.push(
            ScriptedResponse::html("POST /bucket/signin HTTP/1.1", "200 OK", "")
                .with_header("set-cookie", "session=fixture; HttpOnly; Path=/"),
        );
    }
    let status = match mode {
        "expired" => "403 Forbidden",
        "redirect-login" => "302 Found",
        "http" => "500 Internal Server Error",
        _ => "200 OK",
    };
    let partial = PAGE.replace(
        "class=\"commit-comment-box inline-comment\" id=\"discussion_r2\"",
        "class=\"new-review-box\" id=\"discussion_r2\"",
    );
    let whitespace = PAGE.replace("new file.txt", " new file.txt ");
    let mut page = ScriptedResponse::html(
        &format!("GET {WEB} HTTP/1.1"),
        status,
        if mode == "bad-layout" {
            "<html>unknown</html>"
        } else if mode == "partial-layout" {
            &partial
        } else if mode == "public-whitespace" {
            &whitespace
        } else {
            PAGE
        },
    );
    if mode == "redirect-login" {
        page = page.with_header("location", "/bucket/signin");
    }
    frames.push(page);
    if mode == "redirect-login" {
        frames.push(ScriptedResponse::html(
            "GET /bucket/signin HTTP/1.1",
            "200 OK",
            "<form>Sign in</form>",
        ));
    }
    if !matches!(
        mode,
        "bad-layout" | "partial-layout" | "expired" | "http" | "redirect-login"
    ) {
        if mode != "review-only" {
            frames.push(get(
                "/bucket/api/v3/repos/alice/project/issues/1/comments?per_page=100",
                &json!([{"id":1,"body":"ordinary body","user":{"login":"alice"}}]),
            ));
        }
        if mode == "empty" {
            frames.push(get(PR, &Value::Null));
            frames.push(get(
                "/bucket/api/v3/repos/alice/project/pulls?state=all",
                &json!([raw]),
            ));
        } else {
            frames.push(get(
                PR,
                &pr(&if mode == "head-changed" {
                    "b".repeat(40)
                } else {
                    "a".repeat(40)
                }),
            ));
        }
    }
    with_verified_web_session(frames)
}

fn run(
    mode: &str,
    view: bool,
) -> (
    std::process::Output,
    Vec<support::mock_http::CapturedRequest>,
) {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(script(mode));
    let mut command = env.repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project");
    command
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "fixture-password")
        .arg("--json-errors");
    if view {
        command.args(["pr", "view", "1", "--comments"]);
    } else {
        command.args(["pr", "comment", "list", "1"]);
    }
    if mode.starts_with("public") {
        command.env_remove("GB_USER").env_remove("GB_PASSWORD");
    }
    command.args([
        "--kind",
        if mode == "review-only" {
            "review"
        } else {
            "all"
        },
        "--json",
    ]);
    let output = command.output().unwrap();
    (output, server.join().unwrap())
}
fn normalize_target(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (name, value) in object {
                if matches!(name.as_str(), "instance" | "html_url") {
                    if let Some(text) = value.as_str() {
                        let url = url::Url::parse(text).unwrap();
                        *value = json!(format!(
                            "https://example.test{}{}",
                            url.path(),
                            url.fragment()
                                .map(|fragment| format!("#{fragment}"))
                                .unwrap_or_default()
                        ));
                    }
                } else {
                    normalize_target(value);
                }
            }
        }
        Value::Array(array) => {
            for value in array {
                normalize_target(value);
            }
        }
        _ => {}
    }
}

#[test]
fn view_and_list_share_review_collection_with_typed_ids_and_explicit_limits() {
    let (list, requests) = run("all", false);
    assert_success(&list);
    let mut list: Value = serde_json::from_slice(&list.stdout).unwrap();
    let (view, _) = run("all", true);
    assert_success(&view);
    let mut view: Value = serde_json::from_slice(&view.stdout).unwrap();
    normalize_target(&mut list);
    normalize_target(&mut view);
    assert_eq!(view["comments"], list["comments"]);
    assert_eq!(view["review_comments"], list["review_comments"]);
    assert_eq!(view["comment_coverage"], list["comment_coverage"]);
    assert_eq!(list["comments"][0]["id"], 1);
    assert_eq!(
        list["review_comments"][0]["identity"],
        json!({"kind":"inline","instance":"https://example.test/bucket","repository":"alice/project","id":1})
    );
    assert_eq!(list["review_comments"].as_array().unwrap().len(), 5);
    assert!(list["review_comments"][0]["pull_request"].is_null());
    assert_eq!(list["review_comments"][0]["visible_on_pull_request"], 1);
    assert_eq!(
        list["comment_coverage"]["review"]["status"],
        "complete_for_scope"
    );
    assert!(list["comment_coverage"]["historical_review_complete"].is_null());
    assert_eq!(
        list["review_comments"][0]["body"]["format"],
        "rendered_html"
    );
    assert!(list["review_comments"][0]["body"]["markdown"].is_null());
    assert_eq!(requests.iter().filter(|r| r.method != "GET").count(), 1); // Sign-in only.
    let web = requests
        .iter()
        .find(|r| r.target == WEB && r.headers.contains_key("cookie"))
        .unwrap();
    assert!(!web.headers.contains_key("authorization"));
    assert!(web
        .headers
        .get("cookie")
        .unwrap()
        .contains("session=fixture"));
}
#[test]
fn explicit_review_filter_does_not_fetch_discussion_and_empty_fork_details_can_recover() {
    let (output, requests) = run("review-only", false);
    assert_success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["comments"], json!([]));
    assert_eq!(
        value["comment_coverage"]["discussion"]["status"],
        "not_requested"
    );
    assert!(!requests
        .iter()
        .any(|r| r.target.contains("/issues/1/comments")));
    let (output, _) = run("empty", true);
    assert_success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value["head"].is_null());
    assert_eq!(value["review_comments"].as_array().unwrap().len(), 5);
}
#[test]
fn layout_auth_http_and_head_change_fail_without_partial_json_or_mutation() {
    for (mode, code, cause) in [
        (
            "bad-layout",
            "review_comments_unavailable",
            Some("unsupported_markup"),
        ),
        ("expired", "auth_error", None),
        ("redirect-login", "auth_error", None),
        ("http", "api_error", None),
        ("public-http", "api_error", None),
        (
            "head-changed",
            "review_comments_unavailable",
            Some("head_changed"),
        ),
    ] {
        let (output, requests) = run(mode, false);
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], code);
        if let Some(cause) = cause {
            assert_eq!(error["error"]["cause"], cause);
        }
        if mode == "http" {
            assert_eq!(error["error"]["status"], 500);
        }
        assert_eq!(
            requests.iter().filter(|r| r.method != "GET").count(),
            if mode == "public-http" { 0 } else { 1 }
        );
    }
}
#[test]
fn default_discussion_keeps_root_array_and_does_not_require_web_credentials() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        get(PR, &json!({"number":1,"title":"ordinary","state":"open"})),
        get(
            "/bucket/api/v3/repos/alice/project/issues/1/comments?per_page=100",
            &json!([{"id":1,"body":"ordinary"}]),
        ),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .args(["pr", "comment", "list", "1", "--json"])
        .output()
        .unwrap();
    assert_success(&output);
    assert!(serde_json::from_slice::<Value>(&output.stdout)
        .unwrap()
        .is_array());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ordinary discussion comments only"));
    assert!(server.join().unwrap().iter().all(|r| r.method == "GET"));
}
#[test]
fn inline_kind_cannot_accidentally_select_a_mutation_or_be_used_without_comments() {
    for args in [
        vec!["pr", "view", "1", "--kind", "all"],
        vec![
            "pr",
            "comment",
            "1",
            "--kind",
            "review",
            "--body",
            "must not post",
        ],
        vec!["pr", "comment", "list", "1", "--kind", "unknown"],
    ] {
        let env = GbTestEnv::new();
        let output = env.command().args(args).output().unwrap();
        assert_failure(&output);
        assert_eq!(output.status.code(), Some(2));
    }
}

#[test]
fn public_reviews_need_no_password_and_do_not_start_a_cookie_session() {
    let (output, requests) = run("public", false);
    assert_success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["review_comments"].as_array().unwrap().len(), 5);
    assert_eq!(
        value["comment_coverage"]["review_access"],
        "token_or_public"
    );
    assert!(value["comment_coverage"]["web_actor"].is_null());
    assert!(requests.iter().all(|request| request.method == "GET"));
}

#[test]
fn view_review_augmentation_does_not_overwrite_existing_server_metadata() {
    let (output, _) = run("collision", true);
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("without losing server data"));
}

#[test]
fn a_partially_recognized_review_group_fails_view_and_list_without_partial_json() {
    for view in [false, true] {
        let (output, requests) = run("partial-layout", view);
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], "review_comments_unavailable");
        assert_eq!(error["error"]["cause"], "unsupported_markup");
        assert!(!requests
            .iter()
            .any(|r| r.target.contains("/issues/1/comments")));
    }
}

#[test]
fn view_and_list_preserve_whitespace_in_rendered_review_paths() {
    for view in [false, true] {
        let (output, requests) = run("public-whitespace", view);
        assert_success(&output);
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            value["review_comments"][0]["position"]["path"],
            " new file.txt "
        );
        assert_eq!(
            value["review_comments"][1]["position"]["path"],
            " new file.txt "
        );
        assert!(requests.iter().all(|request| request.method == "GET"));
    }
}
