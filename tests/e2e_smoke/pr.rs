use super::support::*;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_pr_create_and_merge_against_live_instance() {
    let temp = tempdir().unwrap();
    let repo = required_env("GB_E2E_REPO");

    login(temp.path());

    let (number, branch, file_name) = create_live_pr_fixture(temp.path(), "e2e-pr-merge");

    let view_before = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["pr", "view", &number.to_string()],
    );
    assert!(view_before.contains(&branch), "stdout: {view_before}");

    let merge_stdout = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["pr", "merge", &number.to_string()],
    );
    assert!(
        merge_stdout.contains(&format!("Merged pull request #{number}")),
        "stdout: {merge_stdout}"
    );

    let view_after = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["pr", "view", &number.to_string()],
    );
    assert!(view_after.contains("MERGED"), "stdout: {view_after}");

    let repo_dir = temp.path().join("verify-merge");
    clone_repo_to(temp.path(), &repo, &repo_dir);
    ensure_remote_main(&repo_dir);
    assert!(
        repo_dir.join(&file_name).exists(),
        "file missing after merge"
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_pr_edit_against_live_instance() {
    let temp = tempdir().unwrap();
    let repo = required_env("GB_E2E_REPO");
    let user = required_env("GB_E2E_USER");
    let suffix = unique_suffix();
    let updated_title = format!("E2E PR edited {suffix}");
    let updated_body = format!("Updated PR body {suffix}");

    login(temp.path());

    let (number, _, _) = create_live_pr_fixture(temp.path(), "e2e-pr-edit");

    let edit_stdout = gb_output_with_env(
        temp.path(),
        temp.path(),
        &[
            "pr",
            "edit",
            &number.to_string(),
            "--title",
            &updated_title,
            "--body",
            &updated_body,
            "--add-assignee",
            &user,
            "--state",
            "closed",
            "--web",
        ],
    );
    assert!(
        edit_stdout.contains(&format!("Updated pull request #{number}: {updated_title}")),
        "stdout: {edit_stdout}"
    );

    let api_stdout = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["api", &format!("repos/{repo}/pulls/{number}")],
    );
    let payload: Value = serde_json::from_str(&api_stdout).unwrap();
    assert_eq!(payload["title"], updated_title);
    assert_eq!(payload["body"], updated_body);
    assert_eq!(payload["state"], "closed");
    assert!(
        payload["assignees"]
            .as_array()
            .unwrap()
            .iter()
            .any(|assignee| assignee["login"] == user),
        "payload: {api_stdout}"
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_pr_comment_and_view_comments_against_live_instance() {
    let temp = tempdir().unwrap();
    let comment_body = format!("pr comment body {}", unique_suffix());

    login(temp.path());

    let (number, branch, _) = create_live_pr_fixture(temp.path(), "e2e-pr-comment");
    let comment_stdout = add_pr_comment(temp.path(), number, &comment_body);
    assert!(
        comment_stdout.contains("Added comment")
            && comment_stdout.contains(&format!("on PR #{number}")),
        "stdout: {comment_stdout}"
    );

    let view_without_comments = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["pr", "view", &number.to_string()],
    );
    assert!(
        view_without_comments.contains(&branch),
        "stdout: {view_without_comments}"
    );
    assert!(
        !view_without_comments.contains("--- Comments ---"),
        "stdout: {view_without_comments}"
    );
    assert!(
        !view_without_comments.contains(&comment_body),
        "stdout: {view_without_comments}"
    );

    let view_with_comments = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["pr", "view", &number.to_string(), "--comments"],
    );
    assert!(
        view_with_comments.contains("--- Comments ---"),
        "stdout: {view_with_comments}"
    );
    assert!(
        view_with_comments.contains(&comment_body),
        "stdout: {view_with_comments}"
    );
    assert!(
        view_with_comments.contains(&required_env("GB_E2E_USER")),
        "stdout: {view_with_comments}"
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_pr_checkout_against_live_instance() {
    let temp = tempdir().unwrap();
    let repo = required_env("GB_E2E_REPO");

    login(temp.path());

    let (number, _, _) = create_live_pr_fixture(temp.path(), "e2e-pr-checkout");
    let repo_dir = temp.path().join("checkout-target");
    clone_repo_to(temp.path(), &repo, &repo_dir);
    ensure_remote_main(&repo_dir);
    let main_before = git_output(&repo_dir, &["rev-parse", "main"]);

    let checkout_stdout = gb_output_with_env(
        temp.path(),
        &repo_dir,
        &["pr", "checkout", &number.to_string()],
    );
    assert!(
        checkout_stdout.contains(&format!("pr-{number}")),
        "stdout: {checkout_stdout}"
    );

    let current_branch = git_output(&repo_dir, &["branch", "--show-current"]);
    assert_eq!(current_branch, format!("pr-{number}"));

    let main_after = git_output(&repo_dir, &["rev-parse", "main"]);
    assert_eq!(
        main_before, main_after,
        "local main branch changed unexpectedly"
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_pr_diff_against_live_instance() {
    let temp = tempdir().unwrap();
    let repo = required_env("GB_E2E_REPO");

    login(temp.path());

    let (number, _, file_name) = create_live_pr_fixture(temp.path(), "e2e-pr-diff");
    let repo_dir = temp.path().join("diff-target");
    clone_repo_to(temp.path(), &repo, &repo_dir);
    ensure_remote_main(&repo_dir);

    let diff_stdout =
        gb_output_with_env(temp.path(), &repo_dir, &["pr", "diff", &number.to_string()]);
    assert!(diff_stdout.contains(&file_name), "stdout: {diff_stdout}");
}
