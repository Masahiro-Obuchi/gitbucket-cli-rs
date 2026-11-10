mod support;
use serde_json::{json, Value};
use support::{
    gb_cmd::{assert_failure, assert_success, GbTestEnv},
    git_repo::{git_output, init_work_repo, run_git},
    mock_http::{
        spawn_scripted_server, with_verified_web_session, CapturedRequest, ScriptedResponse,
    },
};
const PR: &str = "/bucket/api/v3/repos/alice/project/pulls/1";
const WEB: &str = "/bucket/alice/project/pull/1";
const EDIT: &str = "/bucket/alice/project/commit_comments/_data/1?dataType=html";
fn html(method: &str, path: &str, status: &str, body: &str) -> ScriptedResponse {
    ScriptedResponse::html(&format!("{method} {path} HTTP/1.1"), status, body)
}
fn get_pr(base: &str, head: &str) -> ScriptedResponse {
    let value = json!({
        "number": 1,
        "title": "PR",
        "state": "open",
        "base": {"ref": "main", "sha": base},
        "head": {"ref": "topic", "sha": head},
    });
    ScriptedResponse::json(&format!("GET {PR} HTTP/1.1"), "200 OK", &value.to_string())
}
fn edit_form(id: u64, body: &str) -> String {
    format!("<textarea name=content>{body}</textarea><input class=update-comment-{id} value='Update comment'>")
}
fn page(with_review: bool, author: &str) -> String {
    let fragment = fragment()
        .replace("/alice\"", "/AUTHOR\"")
        .replace(">alice</a>", ">AUTHOR</a>")
        .replace("AUTHOR", author);
    let reviews = if with_review {
        format!("<div class=panel><div class=panel-heading><span class=monospace>new file.txt</span></div><div class=fold-comments-1><div class=panel-body>{fragment}</div></div></div>")
    } else {
        String::new()
    };
    format!("<ul id=pullreq-tab><li class=active><a href='/alice/project/pull/1'>Conversation</a></li></ul><div id=comment-list><div class='panel issue-comment-box'><div id=issueContent>PR</div></div>{reviews}</div>")
}
fn fragment() -> String {
    use scraper::{Html, Selector};
    Html::parse_document(include_str!(
        "fixtures/gitbucket-4.44-review-conversation.html"
    ))
    .select(&Selector::parse("#discussion_r1").unwrap())
    .next()
    .unwrap()
    .html()
}
fn fixture() -> (GbTestEnv, String, String) {
    let env = GbTestEnv::new();
    init_work_repo(env.path());
    std::fs::write(env.path().join("README.md"), "base\n").unwrap();
    run_git(env.path(), &["add", "."]);
    run_git(env.path(), &["commit", "-m", "base"]);
    let base = git_output(env.path(), &["rev-parse", "HEAD"]);
    std::fs::write(env.path().join("new file.txt"), "inserted\n").unwrap();
    run_git(env.path(), &["add", "."]);
    run_git(env.path(), &["commit", "-m", "head"]);
    let head = git_output(env.path(), &["rev-parse", "HEAD"]);
    (env, base, head)
}
fn script(base: &str, head: &str, mode: &str) -> Vec<ScriptedResponse> {
    let pr = || get_pr(base, head);
    let mut frames = vec![
        pr(),
        pr(),
        html("POST", "/bucket/signin", "200 OK", "")
            .with_header("set-cookie", "session=test; Path=/"),
        html("GET", WEB, "200 OK", &page(false, "alice")),
        if mode == "head-before" {
            get_pr(base, &"b".repeat(40))
        } else {
            pr()
        },
    ];
    if mode == "head-before" {
        return with_verified_web_session(frames);
    }
    frames.push(pr());
    let url = format!("/bucket/alice/project/commit/{head}/comment/_data/new");
    let post = match mode {
        "rejected" => html("POST", &url, "403 Forbidden", "denied"),
        "post-500" => html("POST", &url, "500 Internal Server Error", "failure"),
        "cut" | "cut-unverified" => html("POST", &url, "200 OK", "6\r\nbroken\r\n")
            .with_header("transfer-encoding", "chunked"),
        "wrong-id" => html(
            "POST",
            &url,
            "200 OK",
            "<div class=commit-comment-box id=discussion_r2>submitted</div>",
        ),
        _ => html("POST", &url, "200 OK", &fragment()),
    };
    frames.push(post);
    if mode == "rejected" {
        return with_verified_web_session(frames);
    }
    if matches!(mode, "verify-500" | "post-500" | "cut-unverified") {
        frames.push(html(
            "GET",
            WEB,
            "500 Internal Server Error",
            "failed readback",
        ));
        return with_verified_web_session(frames);
    }
    frames.push(html("GET", WEB, "200 OK", &page(true, "alice")));
    if mode == "wrong-id" {
        return with_verified_web_session(frames);
    }
    frames.push(html(
        "GET",
        EDIT,
        "200 OK",
        &edit_form(
            1,
            if mode == "wrong-body" {
                "other body"
            } else {
                "new body"
            },
        ),
    ));
    if mode != "wrong-body" {
        frames.push(if mode == "head-after" {
            get_pr(base, &"b".repeat(40))
        } else {
            pr()
        });
    }
    with_verified_web_session(frames)
}
fn invoke(env: &GbTestEnv, port: u16, extra: &[&str]) -> std::process::Output {
    invoke_at(env, port, extra, env.path())
}
fn invoke_at(
    env: &GbTestEnv,
    port: u16,
    extra: &[&str],
    directory: &std::path::Path,
) -> std::process::Output {
    let file = extra
        .windows(2)
        .find(|v| v[0] == "--file")
        .map(|v| v[1])
        .unwrap_or("new file.txt");
    let line = extra
        .windows(2)
        .find(|v| v[0] == "--line")
        .map(|v| v[1])
        .unwrap_or("1");
    let mut command = env.repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project");
    command
        .current_dir(directory)
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "password")
        .args([
            "--json-errors",
            "pr",
            "comment",
            "1",
            "--file",
            file,
            "--line",
            line,
            "--side",
            "new",
            "--web",
            "--body",
            "new body",
            "--json",
        ]);
    if let Some(commit) = extra.windows(2).find(|v| v[0] == "--commit") {
        command.args(["--commit", commit[1]]);
    }
    command.output().unwrap()
}

#[test]
#[cfg(unix)] // ':' is a valid Git path on Unix, but not a Windows filename.
fn root_relative_literal_path_is_verified_from_a_subdirectory() {
    let (env, base, _) = fixture();
    run_git(env.path(), &["mv", "new file.txt", ":(glob)literal.txt"]);
    run_git(env.path(), &["commit", "-m", "literal path"]);
    let head = git_output(env.path(), &["rev-parse", "HEAD"]);
    let frames = script(&base, &head, "success")
        .into_iter()
        .map(|mut frame| {
            frame.body = frame.body.replace("new file.txt", ":(glob)literal.txt");
            frame
        })
        .collect();
    let nested = env.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    let (port, server) = spawn_scripted_server(frames);
    let output = invoke_at(&env, port, &["--file", ":(glob)literal.txt"], &nested);
    assert_success(&output);
    assert_eq!(posts(&server.join().unwrap()), 1);
}
fn posts(requests: &[CapturedRequest]) -> usize {
    requests
        .iter()
        .filter(|r| r.method == "POST" && !r.target.ends_with("signin"))
        .count()
}
#[test]
fn create_uses_saved_coordinates_cookie_actor_and_verified_raw_body() {
    let (env, base, head) = fixture();
    let (port, server) = spawn_scripted_server(script(&base, &head, "success"));
    let output = invoke(&env, port, &[]);
    assert_success(&output);
    let requests = server.join().unwrap();
    assert_eq!(posts(&requests), 1);
    let request = requests
        .iter()
        .find(|r| r.target.ends_with("/_data/new"))
        .unwrap();
    assert!(!request.headers.contains_key("authorization"));
    assert!(request.headers.contains_key("cookie"));
    let fields: std::collections::HashMap<_, _> =
        url::form_urlencoded::parse(request.body.as_bytes())
            .into_owned()
            .collect();
    assert_eq!(fields["issueId"], "1");
    assert_eq!(fields["fileName"], "new file.txt");
    assert_eq!(fields["newLineNumber"], "1");
    assert!(!fields.contains_key("oldLineNumber"));
    assert_eq!(
        serde_json::from_str::<Value>(&fields["diff"]).unwrap()[0]["type"],
        "insert"
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["identity"]["kind"], "inline");
    assert_eq!(value["status"], "succeeded");
    let mut expected: Value =
        serde_json::from_str(include_str!("fixtures/json/review-write.json")).unwrap();
    let mut expected_review: Value =
        serde_json::from_str(include_str!("fixtures/json/review-comment.json")).unwrap();
    let instance = format!("http://127.0.0.1:{port}/bucket");
    expected["identity"]["instance"] = json!(instance);
    expected_review["identity"]["instance"] = json!(instance);
    expected_review["html_url"] = json!(format!("{instance}/alice/project/pull/1#discussion_r1"));
    expected_review["position"]["commit_id"] = json!(head);
    expected["review"] = expected_review;
    expected["base_commit_id"] = json!(base);
    expected["head_commit_id"] = json!(head);
    assert_eq!(value, expected);
    let receipts = std::fs::read_dir(env.path().join("review-writes"))
        .unwrap()
        .map(|p| std::fs::read_to_string(p.unwrap().path()).unwrap())
        .collect::<String>();
    assert!(!receipts.contains("new body"));
    assert!(!receipts.contains("password"));
}

#[test]
fn unicode_repository_names_preserve_exact_authenticated_readback_routes() {
    let (env, base, head) = fixture();
    let frames = script(&base, &head, "success")
        .into_iter()
        .map(|mut frame| {
            frame.expected_request_line = frame
                .expected_request_line
                .replace("/alice/project/", "/alice/project%E5%90%8D/");
            frame.body = frame.body.replace("/alice/project/", "/alice/project名/");
            frame
        })
        .collect();
    let (port, server) = spawn_scripted_server(frames);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project名")
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "password")
        .args([
            "pr",
            "comment",
            "1",
            "--file",
            "new file.txt",
            "--line",
            "1",
            "--side",
            "new",
            "--web",
            "--body",
            "new body",
            "--json",
        ])
        .output()
        .unwrap();
    assert_success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["identity"]["repository"], "alice/project名");
    assert_eq!(posts(&server.join().unwrap()), 1);
}

#[test]
fn closing_or_merging_the_pr_before_submission_prevents_a_new_inline_write() {
    for merged in [false, true] {
        let (env, base, head) = fixture();
        let mut frames = script(&base, &head, "success");
        let index = frames
            .iter()
            .enumerate()
            .filter(|(_, frame)| frame.expected_request_line == format!("GET {PR} HTTP/1.1"))
            .nth(3)
            .unwrap()
            .0;
        let mut pr: Value = serde_json::from_str(&frames[index].body).unwrap();
        pr["state"] = json!(if merged { "open" } else { "closed" });
        pr["merged"] = json!(merged);
        frames[index].body = pr.to_string();
        frames.truncate(index + 1);
        let (port, server) = spawn_scripted_server(frames);
        let output = invoke(&env, port, &[]);
        assert_failure(&output);
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], "review_write_unavailable");
        assert!(error["error"]["operations"].is_null());
        assert_eq!(posts(&server.join().unwrap()), 0);
    }
}
#[test]
fn ambiguous_response_is_reconciled_once_without_reposting() {
    let (env, base, head) = fixture();
    let (port, server) = spawn_scripted_server(script(&base, &head, "cut"));
    let output = invoke(&env, port, &[]);
    assert_success(&output);
    assert_eq!(posts(&server.join().unwrap()), 1);
}
#[test]
fn rejection_or_failed_verification_never_reports_success_and_retains_transport_failure() {
    for mode in [
        "wrong-body",
        "wrong-id",
        "rejected",
        "verify-500",
        "post-500",
        "cut-unverified",
        "head-before",
        "head-after",
    ] {
        let (env, base, head) = fixture();
        let (port, server) = spawn_scripted_server(script(&base, &head, mode));
        let output = invoke(&env, port, &[]);
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        if mode != "head-before" && mode != "rejected" {
            assert_eq!(error["error"]["outcome"], "unverified", "{mode}: {error}");
        }
        if mode == "rejected" {
            assert_eq!(error["error"]["operations"][0]["status"], "failed");
        }
        if mode == "post-500" {
            assert_eq!(error["error"]["status"], 500);
        }
        if mode == "cut-unverified" {
            assert_eq!(error["error"]["cause"], "body_read_failed");
            assert_eq!(error["error"]["status"], 200);
        }
        assert_eq!(
            posts(&server.join().unwrap()),
            if mode == "head-before" { 0 } else { 1 }
        );
    }
}
#[test]
fn pending_intent_is_only_reconciled_on_retry_and_never_resubmitted() {
    let (env, base, head) = fixture();
    let mut frames = script(&base, &head, "verify-500");
    frames.extend(with_verified_web_session(vec![
        get_pr(&base, &head),
        get_pr(&base, &head),
        html("POST", "/bucket/signin", "200 OK", "")
            .with_header("set-cookie", "session=test; Path=/"),
        html("GET", WEB, "200 OK", &page(true, "alice")),
        get_pr(&base, &head),
        html("GET", WEB, "200 OK", &page(true, "alice")),
        html("GET", EDIT, "200 OK", &edit_form(1, "new body")),
        get_pr(&base, &head),
    ]));
    let (port, server) = spawn_scripted_server(frames);
    assert_failure(&invoke(&env, port, &[]));
    let output = invoke(&env, port, &[]);
    assert_success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["reused"],
        true
    );
    assert_eq!(posts(&server.join().unwrap()), 1);
}
#[test]
fn invalid_line_path_or_expected_commit_fails_before_web_authentication() {
    for extra in [
        vec!["--line", "0"],
        vec!["--line", "2"],
        vec!["--file", " README.md"],
        vec!["--file", "README.md"],
        vec!["--commit", "deadbeef"],
    ] {
        let (env, base, head) = fixture();
        let (port, server) =
            spawn_scripted_server(vec![get_pr(&base, &head), get_pr(&base, &head)]);
        let output = invoke(&env, port, &extra);
        assert_failure(&output);
        assert_eq!(posts(&server.join().unwrap()), 0);
    }
}

#[test]
fn binary_non_utf8_symlink_and_submodule_targets_fail_before_web_authentication() {
    for kind in ["binary", "non-utf8", "symlink", "submodule"] {
        if kind == "symlink" && !cfg!(unix) {
            continue;
        }
        let (env, base, head) = fixture();
        let path = env.path().join("unsafe");
        match kind {
            "binary" => std::fs::write(&path, b"binary\0data\n").unwrap(),
            "non-utf8" => std::fs::write(&path, b"invalid\xfftext\n").unwrap(),
            "symlink" => {
                #[cfg(unix)]
                std::os::unix::fs::symlink("new file.txt", &path).unwrap();
            }
            "submodule" => run_git(
                env.path(),
                &[
                    "update-index",
                    "--add",
                    "--cacheinfo",
                    &format!("160000,{head},unsafe"),
                ],
            ),
            _ => unreachable!(),
        }
        if kind != "submodule" {
            run_git(env.path(), &["add", "unsafe"]);
        }
        run_git(env.path(), &["commit", "-m", "unsupported review target"]);
        let head = git_output(env.path(), &["rev-parse", "HEAD"]);
        let (port, server) =
            spawn_scripted_server(vec![get_pr(&base, &head), get_pr(&base, &head)]);
        let output = invoke(&env, port, &["--file", "unsafe"]);
        assert_failure(&output);
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(
            error["error"]["code"], "review_write_unavailable",
            "{kind}: {error}"
        );
        assert_eq!(posts(&server.join().unwrap()), 0);
    }
}
#[test]
fn inline_operations_require_explicit_kind_id_and_web_without_touching_the_network() {
    for args in [
        vec![
            "pr", "comment", "1", "--file", "x", "--line", "1", "--side", "new",
        ],
        vec![
            "pr", "comment", "1", "--file", "x", "--line", "1", "--side", "invalid", "--web",
        ],
        vec!["pr", "comment", "edit", "1", "--id", "1", "--web"],
        vec![
            "pr",
            "comment",
            "edit",
            "1",
            "--kind",
            "discussion",
            "--id",
            "1",
            "--web",
        ],
        vec![
            "pr", "comment", "delete", "1", "--kind", "inline", "--id", "1",
        ],
        vec!["pr", "comment", "reply", "1", "--kind", "inline", "--web"],
    ] {
        let env = GbTestEnv::new();
        let output = env.command().args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
    }
}

fn identified(env: &GbTestEnv, port: u16, action: &str) -> std::process::Output {
    let mut command = env.repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project");
    command
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "password")
        .args([
            "--json-errors",
            "pr",
            "comment",
            action,
            "1",
            "--kind",
            "inline",
            "--id",
            "1",
            "--web",
            "--json",
        ]);
    if action != "delete" {
        command.args(["--body", "new body"]);
    }
    command.output().unwrap()
}
#[test]
fn explicit_id_never_edits_another_author_outdated_or_nonexistent_review() {
    for mode in [
        "other-author",
        "outdated",
        "not-visible",
        "permission",
        "wrong-position",
    ] {
        let (env, base, head) = fixture();
        let mut page = page(
            mode != "not-visible",
            if mode == "other-author" {
                "bob"
            } else {
                "alice"
            },
        );
        if mode == "outdated" {
            page = page.replace("commit-comment-box inline-comment", "commit-comment-box");
        }
        if mode == "wrong-position" {
            page = page.replace("newline=\"1\"", "newline=\"2\"");
        }
        let mut frames = vec![
            get_pr(&base, &head),
            get_pr(&base, &head),
            html("POST", "/bucket/signin", "200 OK", "")
                .with_header("set-cookie", "session=test; Path=/"),
            html("GET", WEB, "200 OK", &page),
        ];
        if mode == "permission" {
            frames.push(html("GET", EDIT, "403 Forbidden", "denied"));
        }
        let (port, server) = spawn_scripted_server(with_verified_web_session(frames));
        let output = identified(&env, port, "edit");
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        assert_eq!(posts(&server.join().unwrap()), 0);
    }
}
#[test]
fn edit_confirms_raw_markdown_and_delete_requires_both_absence_and_404() {
    for mode in [
        "edit",
        "edit-wrong-body",
        "delete",
        "delete-denied-readback",
    ] {
        let (env, base, head) = fixture();
        let action = if mode.starts_with("edit") {
            "edit"
        } else {
            "delete"
        };
        let mut frames = vec![
            get_pr(&base, &head),
            get_pr(&base, &head),
            html("POST", "/bucket/signin", "200 OK", "")
                .with_header("set-cookie", "session=test; Path=/"),
            html("GET", WEB, "200 OK", &page(true, "alice")),
            html("GET", EDIT, "200 OK", &edit_form(1, "old body")),
            get_pr(&base, &head),
            html(
                "POST",
                &format!("/bucket/alice/project/commit_comments/{action}/1"),
                "200 OK",
                "{}",
            ),
            html("GET", WEB, "200 OK", &page(action == "edit", "alice")),
        ];
        frames.push(if action == "edit" {
            html(
                "GET",
                EDIT,
                "200 OK",
                &edit_form(
                    1,
                    if mode == "edit-wrong-body" {
                        "wrong"
                    } else {
                        "new body"
                    },
                ),
            )
        } else {
            html(
                "GET",
                EDIT,
                if mode == "delete" {
                    "404 Not Found"
                } else {
                    "403 Forbidden"
                },
                "",
            )
        });
        if matches!(mode, "edit" | "delete") {
            frames.push(get_pr(&base, &head));
        }
        let (port, server) = spawn_scripted_server(with_verified_web_session(frames));
        let output = identified(&env, port, action);
        if matches!(mode, "edit" | "delete") {
            assert_success(&output);
        } else {
            assert_failure(&output);
            let error: Value = serde_json::from_slice(&output.stderr).unwrap();
            assert_eq!(error["error"]["outcome"], "unverified");
        }
        assert_eq!(posts(&server.join().unwrap()), 1);
    }
}

#[test]
fn editing_an_already_matching_body_is_read_only() {
    let (env, base, head) = fixture();
    let frames = with_verified_web_session(vec![
        get_pr(&base, &head),
        get_pr(&base, &head),
        html("POST", "/bucket/signin", "200 OK", "")
            .with_header("set-cookie", "session=test; Path=/"),
        html("GET", WEB, "200 OK", &page(true, "alice")),
        html("GET", EDIT, "200 OK", &edit_form(1, "new body")),
        get_pr(&base, &head),
    ]);
    let (port, server) = spawn_scripted_server(frames);
    let output = identified(&env, port, "edit");
    assert_success(&output);
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["action"], "edit");
    assert_eq!(result["reused"], true);
    assert_eq!(posts(&server.join().unwrap()), 0);
}
#[test]
fn reply_to_another_author_adds_at_the_validated_group_and_does_not_edit_the_parent() {
    let (env, base, head) = fixture();
    let before = page(true, "bob");
    let second = fragment()
        .replace("discussion_r1", "discussion_r2")
        .replace("commit-comment-1", "commit-comment-2")
        .replace("commit-commentContent-1", "commit-commentContent-2");
    let after = before.replace(
        &fragment()
            .replace("/alice\"", "/bob\"")
            .replace(">alice</a>", ">bob</a>"),
        &format!(
            "{}{}",
            fragment()
                .replace("/alice\"", "/bob\"")
                .replace(">alice</a>", ">bob</a>"),
            second
        ),
    );
    let frames = with_verified_web_session(vec![
        get_pr(&base, &head),
        get_pr(&base, &head),
        html("POST", "/bucket/signin", "200 OK", "")
            .with_header("set-cookie", "session=test; Path=/"),
        html("GET", WEB, "200 OK", &before),
        get_pr(&base, &head),
        get_pr(&base, &head),
        html(
            "POST",
            &format!("/bucket/alice/project/commit/{head}/comment/_data/new"),
            "200 OK",
            &second,
        ),
        html("GET", WEB, "200 OK", &after),
        html(
            "GET",
            "/bucket/alice/project/commit_comments/_data/2?dataType=html",
            "200 OK",
            &edit_form(2, "new body"),
        ),
        get_pr(&base, &head),
    ]);
    let (port, server) = spawn_scripted_server(frames);
    let output = identified(&env, port, "reply");
    assert_success(&output);
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["identity"]["id"], 2);
    assert_eq!(result["reply_to"]["id"], 1);
    assert_eq!(result["review"]["thread"]["id"], "fold-comments-1");
    let requests = server.join().unwrap();
    assert_eq!(posts(&requests), 1);
    assert!(!requests.iter().any(|r| r.target.contains("/edit/1")));
}
