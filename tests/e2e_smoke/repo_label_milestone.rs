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
    let unique_suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let label_name = format!("e2e-label-{}-{unique_suffix}", std::process::id());

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
        "--description",
        "Created by E2E",
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
    let unique_suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let title = format!("e2e-milestone-{unique_suffix}");
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
