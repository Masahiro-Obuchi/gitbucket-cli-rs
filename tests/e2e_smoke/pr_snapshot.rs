use super::support::*;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[ignore = "requires the pinned Docker GitBucket fixture"]
fn e2e_closed_pr_uses_saved_commits_after_branch_reuse_and_deletion() {
    let temp = tempdir().unwrap();
    login(temp.path());
    let repo = required_env("GB_E2E_REPO");
    let (number, branch, original_file) = create_live_pr_fixture(temp.path(), "snapshot-closed");
    let number_arg = number.to_string();
    let path = format!("repos/{repo}/pulls/{number}");
    let before: Value = serde_json::from_str(&gb_output_with_env(
        temp.path(),
        temp.path(),
        &["api", &path],
    ))
    .unwrap();
    let saved_head = before["head"]["sha"].as_str().unwrap();
    let saved_base = before["base"]["sha"].as_str().unwrap();
    gb_output_with_env(temp.path(), temp.path(), &["pr", "close", &number_arg]);
    let work = temp.path().join(format!("work-{branch}"));
    let later_file = format!("{branch}-later.txt");
    std::fs::write(work.join(&later_file), "not part of the closed PR\n").unwrap();
    run_git(&work, &["add", &later_file]);
    run_git(&work, &["commit", "-m", "Reuse the closed PR branch"]);
    run_git(&work, &["push", "origin", &branch]);
    let after: Value = serde_json::from_str(&gb_output_with_env(
        temp.path(),
        temp.path(),
        &["api", &path],
    ))
    .unwrap();
    assert_eq!(after["head"]["sha"], saved_head);
    assert_eq!(after["base"]["sha"], saved_base);
    assert_eq!(
        git_output(
            &work,
            &["ls-remote", "origin", &format!("refs/pull/{number}/head")]
        )
        .split_whitespace()
        .next(),
        Some(saved_head)
    );

    let local = temp.path().join("snapshot-target");
    clone_repo_to(temp.path(), &repo, &local);
    ensure_remote_main(&local);
    let diff = gb_output_with_env(
        temp.path(),
        &local,
        &["pr", "diff", &number_arg, "--no-pager"],
    );
    assert!(diff.contains(&original_file), "{diff}");
    assert!(
        !diff.contains(&later_file),
        "diff included commits created after the PR closed"
    );
    run_git(&work, &["push", "origin", "--delete", &branch]);
    gb_output_with_env(temp.path(), &local, &["pr", "checkout", &number_arg]);
    assert_eq!(git_output(&local, &["rev-parse", "HEAD"]), saved_head);
    assert!(!local.join(&later_file).exists());
}

#[test]
#[ignore = "requires the pinned Docker GitBucket fixture"]
fn e2e_merged_pr_keeps_its_saved_comparison_after_both_branches_advance() {
    let temp = tempdir().unwrap();
    login(temp.path());
    let repo = required_env("GB_E2E_REPO");
    let suffix = unique_suffix();
    let base = format!("snapshot-base-{suffix}");
    let branch = format!("snapshot-merged-{suffix}");
    let work = temp.path().join("work");
    clone_repo_to(temp.path(), &repo, &work);
    ensure_remote_main(&work);
    run_git(&work, &["checkout", "-b", &base]);
    run_git(&work, &["push", "origin", &base]);
    run_git(&work, &["checkout", "-b", &branch]);
    let original = format!("original-{suffix}.txt");
    std::fs::write(work.join(&original), "original merged PR\n").unwrap();
    run_git(&work, &["add", &original]);
    run_git(&work, &["commit", "-m", "original merged PR"]);
    run_git(&work, &["push", "origin", &branch]);
    let number = parse_pr_number(&gb_output_with_env(
        temp.path(),
        &work,
        &[
            "pr",
            "create",
            "--head",
            &branch,
            "--base",
            &base,
            "--title",
            "snapshot merged",
            "--body",
            "saved comparison",
        ],
    ));
    let number_arg = number.to_string();
    let saved_head = git_output(&work, &["rev-parse", "HEAD"]);
    gb_output_with_env(temp.path(), &work, &["pr", "merge", &number_arg]);
    let later = format!("later-{suffix}.txt");
    std::fs::write(work.join(&later), "later branch reuse\n").unwrap();
    run_git(&work, &["add", &later]);
    run_git(&work, &["commit", "-m", "reuse merged branch"]);
    run_git(&work, &["push", "origin", &branch]);
    let local = temp.path().join("local");
    clone_repo_to(temp.path(), &repo, &local);
    ensure_remote_main(&local);
    let diff = gb_output_with_env(
        temp.path(),
        &local,
        &["pr", "diff", &number_arg, "--no-pager"],
    );
    assert!(diff.contains(&original) && !diff.contains(&later), "{diff}");
    gb_output_with_env(temp.path(), &local, &["pr", "checkout", &number_arg]);
    assert_eq!(git_output(&local, &["rev-parse", "HEAD"]), saved_head);
}
