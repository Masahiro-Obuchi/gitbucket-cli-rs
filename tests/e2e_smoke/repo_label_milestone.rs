use super::support::*;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_repo_view_against_live_instance() {
    let temp = tempdir().unwrap();
    let repo = required_env("GB_E2E_REPO");

    login(temp.path());

    let stdout = run_and_assert_success(
        gb_command()
            .current_dir(temp.path())
            .env("GB_CONFIG_DIR", temp.path())
            .env("NO_COLOR", "1")
            .args(["repo", "view", &repo]),
    );

    assert!(stdout.contains(&repo), "stdout: {stdout}");
    assert!(stdout.contains("Visibility:"), "stdout: {stdout}");
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_repo_fork_against_live_instance() {
    let temp = tempdir().unwrap();
    let fork_source = required_env("GB_E2E_FORK_SOURCE");
    let user = required_env("GB_E2E_USER");

    login(temp.path());

    let mut fork_command = gb_command();
    fork_command
        .current_dir(temp.path())
        .args(["repo", "fork", &fork_source]);
    for (key, value) in e2e_env(temp.path()) {
        fork_command.env(key, value);
    }
    let stdout = run_and_assert_success(&mut fork_command);

    assert!(stdout.contains(&fork_source), "stdout: {stdout}");
    assert!(stdout.contains(&format!("→ {user}/")), "stdout: {stdout}");
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_label_list_create_and_delete_against_live_instance() {
    let temp = tempdir().unwrap();
    let suffix = unique_suffix();
    let label_name = format!("e2e-label-{}-{suffix}", std::process::id());

    login(temp.path());

    let mut list_before = gb_command();
    list_before
        .current_dir(temp.path())
        .args(["label", "list", "--json"]);
    for (key, value) in e2e_env(temp.path()) {
        list_before.env(key, value);
    }
    let list_before_stdout = run_and_assert_success(&mut list_before);
    let labels_before: Value = serde_json::from_str(&list_before_stdout).unwrap();
    assert!(
        labels_before.is_array(),
        "label output was not a JSON array: {labels_before}"
    );

    let mut create_command = gb_command();
    create_command.current_dir(temp.path()).args([
        "label",
        "create",
        &label_name,
        "--color",
        "123abc",
    ]);
    for (key, value) in e2e_env(temp.path()) {
        create_command.env(key, value);
    }
    let create_stdout = run_and_assert_success(&mut create_command);
    assert!(
        create_stdout.contains(&label_name),
        "stdout: {create_stdout}"
    );

    let mut list_after_create = gb_command();
    list_after_create
        .current_dir(temp.path())
        .args(["label", "list", "--json"]);
    for (key, value) in e2e_env(temp.path()) {
        list_after_create.env(key, value);
    }
    let list_after_create_stdout = run_and_assert_success(&mut list_after_create);
    let labels_after_create: Value = serde_json::from_str(&list_after_create_stdout).unwrap();
    assert!(
        labels_after_create
            .as_array()
            .unwrap()
            .iter()
            .any(|label| label["name"] == label_name),
        "stdout: {list_after_create_stdout}"
    );

    let mut delete_command = gb_command();
    delete_command
        .current_dir(temp.path())
        .args(["label", "delete", &label_name, "--yes"]);
    for (key, value) in e2e_env(temp.path()) {
        delete_command.env(key, value);
    }
    let delete_stdout = run_and_assert_success(&mut delete_command);
    assert!(
        delete_stdout.contains(&label_name),
        "stdout: {delete_stdout}"
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_milestone_list_create_edit_and_delete_against_live_instance() {
    let temp = tempdir().unwrap();
    let title = format!("e2e-milestone-{}", unique_suffix());
    let updated_title = format!("{title}-updated");

    login(temp.path());

    let mut create_command = gb_command();
    create_command.current_dir(temp.path()).args([
        "milestone",
        "create",
        &title,
        "--description",
        "Created by E2E",
        "--due-on",
        "2026-04-01",
    ]);
    for (key, value) in e2e_env(temp.path()) {
        create_command.env(key, value);
    }
    let create_stdout = run_and_assert_success(&mut create_command);
    assert!(create_stdout.contains(&title), "stdout: {create_stdout}");

    let mut list_command = gb_command();
    list_command
        .current_dir(temp.path())
        .args(["milestone", "list", "--state", "all", "--json"]);
    for (key, value) in e2e_env(temp.path()) {
        list_command.env(key, value);
    }
    let list_stdout = run_and_assert_success(&mut list_command);
    let milestones: Value = serde_json::from_str(&list_stdout).unwrap();
    let number = milestones
        .as_array()
        .unwrap()
        .iter()
        .find(|milestone| milestone["title"] == title)
        .and_then(|milestone| milestone["number"].as_u64())
        .unwrap_or_else(|| {
            panic!("failed to find created milestone in list output: {list_stdout}")
        });

    let mut edit_command = gb_command();
    edit_command.current_dir(temp.path()).args([
        "milestone",
        "edit",
        &number.to_string(),
        "--title",
        &updated_title,
        "--state",
        "closed",
        "--due-on",
        "2026-04-02",
    ]);
    for (key, value) in e2e_env(temp.path()) {
        edit_command.env(key, value);
    }
    let edit_stdout = run_and_assert_success(&mut edit_command);
    assert!(
        edit_stdout.contains(&updated_title),
        "stdout: {edit_stdout}"
    );

    let mut view_command = gb_command();
    view_command
        .current_dir(temp.path())
        .args(["milestone", "view", &number.to_string()]);
    for (key, value) in e2e_env(temp.path()) {
        view_command.env(key, value);
    }
    let view_stdout = run_and_assert_success(&mut view_command);
    assert!(
        view_stdout.contains(&updated_title),
        "stdout: {view_stdout}"
    );
    assert!(view_stdout.contains("CLOSED"), "stdout: {view_stdout}");

    let mut delete_command = gb_command();
    delete_command.current_dir(temp.path()).args([
        "milestone",
        "delete",
        &number.to_string(),
        "--yes",
    ]);
    for (key, value) in e2e_env(temp.path()) {
        delete_command.env(key, value);
    }
    let delete_stdout = run_and_assert_success(&mut delete_command);
    assert!(
        delete_stdout.contains(&format!("#{number}")),
        "stdout: {delete_stdout}"
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_repo_clone_against_live_instance() {
    let temp = tempdir().unwrap();
    let repo = required_env("GB_E2E_REPO");
    let clone_target = temp.path().join("cloned-repo");

    login(temp.path());

    let seed_dir = temp.path().join("seed-for-clone");
    clone_repo_to(temp.path(), &repo, &seed_dir);
    ensure_remote_main(&seed_dir);

    let clone_stdout = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["repo", "clone", &repo, clone_target.to_str().unwrap()],
    );
    assert!(clone_target.join(".git").is_dir(), "repo was not cloned");
    assert!(clone_stdout.is_empty(), "stdout: {clone_stdout}");

    let remote = git_output(&clone_target, &["remote", "get-url", "origin"]);
    assert!(
        remote.contains(&repo),
        "origin remote did not reference repo: {remote}"
    );
    assert!(
        clone_target.join("README.md").exists(),
        "README.md missing after clone"
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_repo_delete_against_live_instance() {
    let temp = tempdir().unwrap();
    let user = required_env("GB_E2E_USER");
    let repo_name = format!("e2e-delete-{}", unique_suffix());
    let full_name = format!("{user}/{repo_name}");

    login(temp.path());

    let create_stdout =
        gb_output_with_env(temp.path(), temp.path(), &["repo", "create", &repo_name]);
    assert!(
        create_stdout.contains(&full_name),
        "stdout: {create_stdout}"
    );

    let delete_stdout = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["repo", "delete", &full_name, "--yes"],
    );
    assert!(
        delete_stdout.contains(&full_name),
        "stdout: {delete_stdout}"
    );

    let mut api_command = gb_command();
    api_command
        .current_dir(temp.path())
        .args(["api", &format!("repos/{full_name}")]);
    for (key, value) in e2e_env(temp.path()) {
        api_command.env(key, value);
    }
    let output = api_command.output().unwrap();
    assert!(
        !output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("404"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_private_repo_clone_uses_configured_token_without_url_credentials() {
    let temp = tempdir().unwrap();
    login(temp.path());
    let name = format!("e2e-private-clone-{}", unique_suffix());
    let repo = format!("{}/{}", required_env("GB_E2E_USER"), name);
    gb_output_with_env(
        temp.path(),
        temp.path(),
        &["repo", "create", &name, "--private", "--add-readme"],
    );
    let destination = temp.path().join("private-clone");
    gb_output_with_env(
        temp.path(),
        temp.path(),
        &["repo", "clone", &repo, destination.to_str().unwrap()],
    );
    assert!(destination.join("README.md").exists());
    let remote = git_output(&destination, &["remote", "get-url", "origin"]);
    let parsed = url::Url::parse(&remote).unwrap();
    assert!(parsed.username().is_empty());
    assert!(parsed.password().is_none());
    let config = std::fs::read_to_string(destination.join(".git/config")).unwrap();
    assert!(!config.contains("credential.helper"));
    assert!(!config.contains(&required_env("GB_E2E_TOKEN")));
    gb_output_with_env(
        temp.path(),
        temp.path(),
        &["repo", "delete", &repo, "--yes"],
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_origin_repository_resolution_matches_configured_instance() {
    let temp = tempdir().unwrap();
    login(temp.path());
    let repo = required_env("GB_E2E_REPO");
    let destination = temp.path().join("origin-resolution");
    clone_repo_to(temp.path(), &repo, &destination);
    let mut command = gb_command();
    command
        .current_dir(&destination)
        .args(["issue", "list", "--json"]);
    for (key, value) in e2e_env(temp.path()) {
        command.env(key, value);
    }
    command
        .env_remove("GB_REPO")
        .env_remove("GB_PROFILE")
        .env("GB_HOST", required_env("GB_E2E_HOST"));
    let stdout = run_and_assert_success(&mut command);
    let payload: Value = serde_json::from_str(&stdout).unwrap();
    assert!(payload.is_array());
}

fn milestone_run(temp: &std::path::Path, args: &[&str], web: bool) -> String {
    let mut command = gb_command();
    command
        .current_dir(temp)
        .args(args)
        .stdin(std::process::Stdio::null());
    for (key, value) in e2e_env(temp) {
        command.env(key, value);
    }
    if !web {
        command.env_remove("GB_PASSWORD");
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "args: {args:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !web {
        assert!(!String::from_utf8_lossy(&output.stderr).contains("using the Web route"));
    }
    String::from_utf8(output.stdout).unwrap()
}

fn milestone_stored(temp: &std::path::Path, number: u64) -> Value {
    let path = format!("repos/{}/milestones/{number}", required_env("GB_E2E_REPO"));
    serde_json::from_str(&milestone_run(temp, &["api", &path], false)).unwrap()
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance"]
fn e2e_milestone_partial_edits_preserve_every_other_field_without_web_password() {
    let temp = tempdir().unwrap();
    login(temp.path());
    let title = format!("preserve-{}", unique_suffix());
    let created = milestone_run(temp.path(), &["milestone", "create", &title], false);
    let number = parse_issue_number(&created);
    let number_arg = number.to_string();
    let path = format!("repos/{}/milestones/{number}", required_env("GB_E2E_REPO"));
    let seed = temp.path().join("seed.json");
    std::fs::write(&seed, serde_json::json!({"title": title,"state":"closed","description":" keep\nthese details ","due_on":"2027-04-01T09:30:00Z"}).to_string()).unwrap();
    milestone_run(
        temp.path(),
        &[
            "api",
            &path,
            "-X",
            "PATCH",
            "--input",
            seed.to_str().unwrap(),
        ],
        false,
    );
    let mut expected = milestone_stored(temp.path(), number);
    assert_eq!(expected["due_on"], "2027-04-01T09:30:00Z");
    for (flag, value, key, expected_value) in [
        (
            "--title",
            "preserved-title",
            "title",
            serde_json::json!("preserved-title"),
        ),
        (
            "--description",
            "new description",
            "description",
            serde_json::json!("new description"),
        ),
        ("--state", "open", "state", serde_json::json!("open")),
        ("--state", "closed", "state", serde_json::json!("closed")),
        (
            "--due-on",
            "2027-05-02",
            "due_on",
            serde_json::json!("2027-05-02T00:00:00Z"),
        ),
        ("--due-on", "", "due_on", Value::Null),
        ("--description", "", "description", serde_json::json!("")),
        (
            "--title",
            "null-fields-retained",
            "title",
            serde_json::json!("null-fields-retained"),
        ),
    ] {
        milestone_run(
            temp.path(),
            &["milestone", "edit", &number_arg, flag, value],
            false,
        );
        expected[key] = expected_value;
        let stored = milestone_stored(temp.path(), number);
        for field in ["title", "state", "description", "due_on"] {
            assert_eq!(
                stored[field], expected[field],
                "{flag} {value:?} changed {field}"
            );
        }
        if key != "state" {
            assert_eq!(stored["closed_at"], expected["closed_at"]);
        }
        expected = stored;
    }
    milestone_run(
        temp.path(),
        &["milestone", "delete", &number_arg, "--yes"],
        false,
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance"]
fn e2e_milestone_unicode_and_space_titles_use_web_only_where_needed() {
    let temp = tempdir().unwrap();
    login(temp.path());
    let title = format!("日本語 release {}", unique_suffix());
    let output = milestone_run(
        temp.path(),
        &["milestone", "create", &title, "--description", "説明"],
        true,
    );
    let number = parse_issue_number(&output);
    let number_arg = number.to_string();
    for (args, key, expected) in [
        (
            vec!["--state", "closed"],
            "state",
            serde_json::json!("closed"),
        ),
        (
            vec!["--description", "\u{a0}更新した説明\u{a0}"],
            "description",
            serde_json::json!("\u{a0}更新した説明\u{a0}"),
        ),
        (
            vec!["--title", "\u{3000}新しい release\u{3000}"],
            "title",
            serde_json::json!("\u{3000}新しい release\u{3000}"),
        ),
        (
            vec!["--due-on", "2027-04-02"],
            "due_on",
            serde_json::json!("2027-04-02T00:00:00Z"),
        ),
        (vec!["--state", "open"], "state", serde_json::json!("open")),
        (
            vec!["--description", "", "--due-on", ""],
            "description",
            serde_json::json!(""),
        ),
    ] {
        let before = milestone_stored(temp.path(), number);
        let mut command = vec!["milestone", "edit", &number_arg];
        command.extend(&args);
        milestone_run(temp.path(), &command, true);
        let stored = milestone_stored(temp.path(), number);
        assert_eq!(stored[key], expected);
        for field in ["title", "state", "description", "due_on"] {
            if field != key && !(args.contains(&"--due-on") && field == "due_on") {
                assert_eq!(stored[field], before[field], "Web edit changed {field}");
            }
        }
    }
    // Switching to a REST-compatible title needs no password, even from Unicode.
    milestone_run(
        temp.path(),
        &[
            "milestone",
            "edit",
            &number_arg,
            "--title",
            &format!("rest-again-{}", unique_suffix()),
        ],
        false,
    );
    milestone_run(
        temp.path(),
        &["milestone", "delete", &number_arg, "--yes"],
        false,
    );
}

fn label_run(temp: &std::path::Path, args: &[&str], web: bool) -> String {
    let mut command = gb_command();
    command
        .current_dir(temp)
        .args(args)
        .stdin(std::process::Stdio::null());
    for (key, value) in e2e_env(temp) {
        command.env(key, value);
    }
    if !web {
        command.env_remove("GB_PASSWORD");
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{args:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !web {
        assert!(!String::from_utf8_lossy(&output.stderr).contains("Notice:"));
    }
    String::from_utf8(output.stdout).unwrap()
}

fn label_reject(temp: &std::path::Path, args: &[&str]) -> String {
    let mut command = gb_command();
    command
        .current_dir(temp)
        .args(args)
        .stdin(std::process::Stdio::null());
    for (key, value) in e2e_env(temp) {
        command.env(key, value);
    }
    command.env_remove("GB_PASSWORD");
    let output = command.output().unwrap();
    assert!(!output.status.success(), "{args:?} unexpectedly succeeded");
    assert!(output.stdout.is_empty());
    String::from_utf8(output.stderr).unwrap()
}

async fn web_label_id(web: &reqwest::Client, repo: &str, name: &str) -> u64 {
    let response = web
        .get(format!(
            "{}/{repo}/issues/labels",
            required_env("GB_E2E_BASE_URL")
        ))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let page = response.text().await.unwrap();
    let document = scraper::Html::parse_document(&page);
    let anchors = scraper::Selector::parse("a[id^='label-row-content-']").unwrap();
    let ids: Vec<_> = document
        .select(&anchors)
        .filter_map(|anchor| {
            let visible = anchor
                .text()
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if visible != name {
                return None;
            }
            anchor
                .value()
                .attr("id")?
                .strip_prefix("label-row-content-")?
                .parse::<u64>()
                .ok()
        })
        .collect();
    assert_eq!(ids.len(), 1, "could not locate unique label {name:?}");
    ids[0]
}

#[tokio::test]
#[ignore = "requires a Docker-backed GitBucket instance"]
async fn e2e_label_updates_preserve_numeric_identity_and_issue_associations() {
    let temp = tempdir().unwrap();
    login(temp.path());
    let suffix = unique_suffix();
    let name = format!("needs & review-{suffix}");
    let renamed = format!("renamed & label-{suffix}");
    let keep = format!("\u{a0}keep-{suffix}\u{a0}");
    let urgent = format!(" urgent-{suffix} ");
    let repo = required_env("GB_E2E_REPO");
    for label in [&name, &keep, &urgent] {
        label_run(
            temp.path(),
            &["label", "create", label, "--color", "123abc"],
            false,
        );
    }
    let number = create_live_issue(temp.path(), &format!("label-linked-{suffix}"), "keep body");
    let number_arg = number.to_string();
    label_run(
        temp.path(),
        &[
            "issue",
            "edit",
            &number_arg,
            "--add-label",
            &name,
            "--add-label",
            &keep,
        ],
        false,
    );
    let web = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
    let signin = web
        .post(format!("{}/signin", required_env("GB_E2E_BASE_URL")))
        .form(&[
            ("userName", required_env("GB_E2E_USER")),
            ("password", required_env("GB_E2E_PASSWORD")),
            ("hash", String::new()),
        ])
        .send()
        .await
        .unwrap();
    assert!(signin.status().is_success());
    let id = web_label_id(&web, &repo, &name).await;
    label_run(
        temp.path(),
        &["label", "edit", &name, "--name", &renamed],
        false,
    );
    assert_eq!(web_label_id(&web, &repo, &renamed).await, id);
    let mut label_path = url::Url::parse(&format!(
        "{}/api/v3/repos/{repo}/labels/",
        required_env("GB_E2E_BASE_URL")
    ))
    .unwrap();
    label_path
        .path_segments_mut()
        .unwrap()
        .pop_if_empty()
        .push(&renamed);
    let saved: Value = serde_json::from_str(&label_run(
        temp.path(),
        &["api", label_path.as_str()],
        false,
    ))
    .unwrap();
    assert_eq!(saved["color"], "123abc");
    let error = label_reject(
        temp.path(),
        &["label", "edit", &renamed, "--color", "abcdef"],
    );
    assert!(error.contains("--web"), "{error}");
    assert_eq!(web_label_id(&web, &repo, &renamed).await, id);
    label_run(
        temp.path(),
        &["label", "edit", &renamed, "--color", "abcdef", "--web"],
        true,
    );
    assert_eq!(web_label_id(&web, &repo, &renamed).await, id);
    let saved: Value = serde_json::from_str(&label_run(
        temp.path(),
        &["api", label_path.as_str()],
        false,
    ))
    .unwrap();
    assert_eq!(saved["color"], "abcdef");
    let issue_path = format!("repos/{repo}/issues/{number}");
    let linked: Value =
        serde_json::from_str(&label_run(temp.path(), &["api", &issue_path], false)).unwrap();
    assert!(linked["labels"]
        .as_array()
        .unwrap()
        .iter()
        .any(|label| label["name"] == renamed && label["color"] == "abcdef"));
    assert!(linked["labels"]
        .as_array()
        .unwrap()
        .iter()
        .any(|label| label["name"] == keep));
    let rejected_name = format!("must-not-create-{suffix}");
    assert!(label_reject(
        temp.path(),
        &[
            "label",
            "create",
            &rejected_name,
            "--color",
            "123abc",
            "--description",
            "not supported"
        ]
    )
    .contains("description writes are unsupported"));
    assert!(label_reject(
        temp.path(),
        &[
            "label",
            "edit",
            &renamed,
            "--name",
            &rejected_name,
            "--description",
            "not supported"
        ]
    )
    .contains("description writes are unsupported"));
    assert!(label_reject(
        temp.path(),
        &["label", "edit", &renamed, "--remove-description"]
    )
    .contains("description writes are unsupported"));
    assert_eq!(web_label_id(&web, &repo, &renamed).await, id);
    let all: Value =
        serde_json::from_str(&label_run(temp.path(), &["label", "list", "--json"], false)).unwrap();
    assert!(!all
        .as_array()
        .unwrap()
        .iter()
        .any(|label| label["name"] == rejected_name));
    label_run(
        temp.path(),
        &[
            "issue",
            "edit",
            &number_arg,
            "--add-label",
            &urgent,
            "--remove-label",
            &renamed,
        ],
        false,
    );
    let changed: Value =
        serde_json::from_str(&label_run(temp.path(), &["api", &issue_path], false)).unwrap();
    assert!(changed["labels"]
        .as_array()
        .unwrap()
        .iter()
        .any(|label| label["name"] == keep));
    assert!(changed["labels"]
        .as_array()
        .unwrap()
        .iter()
        .any(|label| label["name"] == urgent));
    assert!(!changed["labels"]
        .as_array()
        .unwrap()
        .iter()
        .any(|label| label["name"] == renamed));
    label_run(
        temp.path(),
        &[
            "issue",
            "edit",
            &number_arg,
            "--title",
            "metadata and labels edited",
            "--add-label",
            &renamed,
            "--remove-label",
            &urgent,
        ],
        true,
    );
    let mixed: Value =
        serde_json::from_str(&label_run(temp.path(), &["api", &issue_path], false)).unwrap();
    assert_eq!(mixed["title"], "metadata and labels edited");
    assert!(mixed["labels"]
        .as_array()
        .unwrap()
        .iter()
        .any(|label| label["name"] == keep));
    assert!(mixed["labels"]
        .as_array()
        .unwrap()
        .iter()
        .any(|label| label["name"] == renamed));
    assert_eq!(web_label_id(&web, &repo, &renamed).await, id);
    for label in [&renamed, &keep, &urgent] {
        label_run(temp.path(), &["label", "delete", label, "--yes"], false);
    }
}
