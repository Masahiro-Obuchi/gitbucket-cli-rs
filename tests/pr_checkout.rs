mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::json;
use support::gb_cmd::{assert_failure, assert_success, stderr, GbTestEnv};
use support::git_repo::{
    clone_branch, commit_readme, commit_tracked_readme, git_output, init_bare_repo, init_work_repo,
    push_main, run_git,
};
use support::mock_http::{spawn_scripted_server, ScriptedResponse};

struct Fixture {
    env: GbTestEnv,
    source: PathBuf,
    local: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let env = GbTestEnv::new();
        let remote = env.path().join("remote.git");
        let source = env.path().join("source");
        let local = env.path().join("local");
        init_bare_repo(&remote);
        init_work_repo(&source);
        commit_readme(&source, "base\n", "base");
        push_main(&source, &remote);
        run_git(&source, &["checkout", "-b", "feature"]);
        commit_tracked_readme(&source, "PR changes\n", "feature");
        run_git(&source, &["push", "origin", "feature"]);
        clone_branch(env.path(), &remote, "main", &local);
        support::git_repo::configure_test_user(&local);
        Self { env, source, local }
    }

    fn response(&self, prefix: &str, repo: &str, head: &str) -> ScriptedResponse {
        ScriptedResponse::json(
            &format!("GET {prefix}/api/v3/repos/{repo}/pulls/7 HTTP/1.1"),
            "200 OK",
            &json!({"number":7,"title":"checkout test","state":"open","head":{"ref":head}})
                .to_string(),
        )
    }

    fn checkout(&self, port: u16, prefix: &str, repo: &str, dir: &Path) -> Output {
        self.env
            .repo_api_command(format!("127.0.0.1:{port}{prefix}"), repo)
            .current_dir(dir)
            .args(["pr", "checkout", "7"])
            .output()
            .unwrap()
    }

    fn add_submodule(&self) -> PathBuf {
        let source = self.env.path().join("submodule-source");
        init_work_repo(&source);
        commit_readme(&source, "submodule base\n", "submodule base");
        run_git(&source, &["branch", "-M", "main"]);
        fs::write(source.join(".gitignore"), "private.txt\n").unwrap();
        run_git(&source, &["add", ".gitignore"]);
        run_git(&source, &["commit", "-m", "ignore private file"]);
        run_git(
            &self.source,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                source.to_str().unwrap(),
                "module",
            ],
        );
        run_git(&self.source, &["commit", "-am", "add submodule"]);
        run_git(&self.source, &["push", "origin", "feature"]);
        run_git(&self.local, &["fetch", "origin", "feature"]);
        run_git(&self.local, &["merge", "--ff-only", "origin/feature"]);
        run_git(
            &self.local,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "update",
                "--init",
            ],
        );
        source
    }

    fn snapshot(&self) -> Vec<String> {
        vec![
            git_output(&self.local, &["rev-parse", "HEAD"]),
            git_output(&self.local, &["symbolic-ref", "HEAD"]),
            git_output(
                &self.local,
                &[
                    "for-each-ref",
                    "--format=%(refname) %(objectname)",
                    "refs/heads",
                ],
            ),
            git_output(&self.local, &["diff", "--binary"]),
            git_output(&self.local, &["diff", "--cached", "--binary"]),
            git_output(&self.local, &["status", "--porcelain=v1"]),
            fs::read_to_string(self.local.join("README.md")).unwrap(),
        ]
    }

    fn refuse_unchanged(&self, port: u16, prefix: &str, repo: &str, message: &str) {
        let before = self.snapshot();
        let output = self.checkout(port, prefix, repo, &self.local);
        assert_failure(&output);
        assert!(!String::from_utf8_lossy(&output.stdout).contains("✓"));
        assert!(stderr(&output).contains(message), "{}", stderr(&output));
        assert_eq!(self.snapshot(), before);
    }
}

#[test]
fn creates_branch_and_reuses_only_matching_target_at_same_commit() {
    let f = Fixture::new();
    let (port, server) = spawn_scripted_server(vec![
        f.response("", "alice/demo", "feature"),
        f.response("", "alice/demo", "feature"),
    ]);
    let main = git_output(&f.local, &["rev-parse", "main"]);
    assert_success(&f.checkout(port, "", "alice/demo", &f.local));
    assert_eq!(git_output(&f.local, &["branch", "--show-current"]), "pr-7");
    let tip = git_output(&f.local, &["rev-parse", "pr-7"]);
    assert_eq!(tip, git_output(&f.source, &["rev-parse", "feature"]));
    run_git(&f.local, &["checkout", "main"]);
    assert_success(&f.checkout(port, "", "alice/demo", &f.local));
    assert_eq!(git_output(&f.local, &["rev-parse", "pr-7"]), tip);
    assert_eq!(git_output(&f.local, &["rev-parse", "main"]), main);
    server.join().unwrap();
}

#[test]
fn refuses_local_commits_without_resetting_them() {
    let f = Fixture::new();
    let (port, server) = spawn_scripted_server(vec![
        f.response("", "alice/demo", "feature"),
        f.response("", "alice/demo", "feature"),
    ]);
    assert_success(&f.checkout(port, "", "alice/demo", &f.local));
    commit_tracked_readme(&f.local, "keep my commit\n", "local change");
    f.refuse_unchanged(port, "", "alice/demo", "Refusing to reset");
    server.join().unwrap();
}

#[test]
fn refuses_unrecorded_legacy_branch_even_at_the_same_commit() {
    let f = Fixture::new();
    run_git(&f.local, &["fetch", "origin", "feature"]);
    run_git(&f.local, &["branch", "pr-7", "origin/feature"]);
    let (port, server) = spawn_scripted_server(vec![f.response("", "alice/demo", "feature")]);
    f.refuse_unchanged(port, "", "alice/demo", "not recorded as belonging");
    server.join().unwrap();
}

#[test]
fn same_number_in_other_repository_deployment_or_host_is_not_reused() {
    let f = Fixture::new();
    let (port, server) = spawn_scripted_server(vec![
        f.response("", "alice/demo", "feature"),
        f.response("", "alice/other", "feature"),
        f.response("/bucket", "alice/demo", "feature"),
    ]);
    assert_success(&f.checkout(port, "", "alice/demo", &f.local));
    f.refuse_unchanged(port, "", "alice/other", "not recorded as belonging");
    f.refuse_unchanged(port, "/bucket", "alice/demo", "not recorded as belonging");
    server.join().unwrap();
    let (port, server) = spawn_scripted_server(vec![f.response("", "alice/demo", "feature")]);
    f.refuse_unchanged(port, "", "alice/demo", "not recorded as belonging");
    server.join().unwrap();
}

#[test]
fn refuses_staged_unstaged_and_untracked_changes() {
    for kind in ["staged", "unstaged", "untracked"] {
        let f = Fixture::new();
        if kind == "untracked" {
            fs::write(f.local.join("untracked.txt"), "keep this\n").unwrap();
        } else {
            fs::write(f.local.join("README.md"), "my uncommitted changes\n").unwrap();
            if kind == "staged" {
                run_git(&f.local, &["add", "README.md"]);
            }
        }
        let (port, server) = spawn_scripted_server(vec![f.response("", "alice/demo", "feature")]);
        f.refuse_unchanged(port, "", "alice/demo", "clean working tree");
        if kind == "untracked" {
            assert_eq!(
                fs::read_to_string(f.local.join("untracked.txt")).unwrap(),
                "keep this\n"
            );
        }
        server.join().unwrap();
    }
}

#[test]
fn refuses_branch_in_another_worktree_and_allows_checkout_from_linked_worktree() {
    let f = Fixture::new();
    let (port, server) = spawn_scripted_server(vec![
        f.response("", "alice/demo", "feature"),
        f.response("", "alice/demo", "feature"),
        f.response("", "alice/demo", "feature"),
    ]);
    assert_success(&f.checkout(port, "", "alice/demo", &f.local));
    run_git(&f.local, &["checkout", "main"]);
    let linked = f.env.path().join("linked worktree");
    run_git(
        &f.local,
        &["worktree", "add", linked.to_str().unwrap(), "pr-7"],
    );
    let linked_before = git_output(&linked, &["rev-parse", "HEAD"]);
    f.refuse_unchanged(port, "", "alice/demo", "another worktree");
    assert_eq!(git_output(&linked, &["rev-parse", "HEAD"]), linked_before);
    assert_success(&f.checkout(port, "", "alice/demo", &linked));
    server.join().unwrap();
}

#[test]
fn advancing_or_force_pushing_pr_never_moves_existing_local_branch() {
    for force in [false, true] {
        let f = Fixture::new();
        let (port, server) = spawn_scripted_server(vec![
            f.response("", "alice/demo", "feature"),
            f.response("", "alice/demo", "feature"),
        ]);
        assert_success(&f.checkout(port, "", "alice/demo", &f.local));
        if force {
            run_git(&f.source, &["reset", "--hard", "main"]);
        }
        commit_tracked_readme(&f.source, "new PR head\n", "remote update");
        run_git(&f.source, &["push", "--force", "origin", "feature"]);
        f.refuse_unchanged(port, "", "alice/demo", "Refusing to reset");
        // Force updates are allowed only on the isolated internal fetch ref.
        let internal = git_output(
            &f.local,
            &[
                "for-each-ref",
                "--format=%(objectname)",
                "refs/gb/pr-targets",
            ],
        );
        assert_eq!(internal, git_output(&f.source, &["rev-parse", "feature"]));
        server.join().unwrap();
    }
}

#[test]
fn fetch_failure_preserves_head_branches_index_and_files() {
    let f = Fixture::new();
    let (port, server) = spawn_scripted_server(vec![f.response("", "alice/demo", "missing-head")]);
    f.refuse_unchanged(port, "", "alice/demo", "git fetch failed");
    server.join().unwrap();
}

#[test]
fn checkout_failure_preserves_head_and_does_not_create_branch() {
    let f = Fixture::new();
    fs::write(f.local.join(".git/index.lock"), "held by another operation").unwrap();
    let (port, server) = spawn_scripted_server(vec![f.response("", "alice/demo", "feature")]);
    f.refuse_unchanged(port, "", "alice/demo", "git checkout failed");
    assert_eq!(
        fs::read_to_string(f.local.join(".git/index.lock")).unwrap(),
        "held by another operation"
    );
    assert!(git_output(&f.local, &["config", "--local", "--list"])
        .lines()
        .all(|line| !line.starts_with("branch.pr-7.gb-pr-target=")));
    server.join().unwrap();
}

#[test]
fn ignored_files_are_not_overwritten_by_checkout() {
    let f = Fixture::new();
    fs::write(f.source.join("private.txt"), "PR file\n").unwrap();
    run_git(&f.source, &["add", "private.txt"]);
    run_git(&f.source, &["commit", "-m", "add private file"]);
    run_git(&f.source, &["push", "origin", "feature"]);
    fs::write(f.local.join(".git/info/exclude"), "private.txt\n").unwrap();
    fs::write(f.local.join("private.txt"), "my ignored data\n").unwrap();
    let (port, server) = spawn_scripted_server(vec![f.response("", "alice/demo", "feature")]);
    f.refuse_unchanged(port, "", "alice/demo", "git checkout failed");
    assert_eq!(
        fs::read_to_string(f.local.join("private.txt")).unwrap(),
        "my ignored data\n"
    );
    server.join().unwrap();
}

#[test]
fn recursive_settings_do_not_update_submodules_or_overwrite_ignored_files() {
    let f = Fixture::new();
    let source = f.add_submodule();
    let module = f.local.join("module");
    let before_refs = git_output(
        &module,
        &["for-each-ref", "--format=%(refname) %(objectname)"],
    );
    let before_head = git_output(&module, &["rev-parse", "HEAD"]);
    let fetch_head = f.local.join(".git/modules/module/FETCH_HEAD");
    fs::write(&fetch_head, "keep submodule fetch head\n").unwrap();
    fs::write(module.join("private.txt"), "my ignored submodule data\n").unwrap();
    fs::write(source.join("private.txt"), "PR file\n").unwrap();
    run_git(&source, &["add", "-f", "private.txt"]);
    run_git(&source, &["commit", "-m", "track private file"]);
    run_git(&f.source.join("module"), &["fetch", "origin", "main"]);
    run_git(
        &f.source.join("module"),
        &["checkout", "--detach", "origin/main"],
    );
    run_git(&f.source, &["commit", "-am", "advance submodule"]);
    run_git(&f.source, &["push", "origin", "feature"]);
    run_git(&f.local, &["config", "submodule.recurse", "true"]);
    run_git(&module, &["config", "protocol.file.allow", "always"]);
    let (port, server) = spawn_scripted_server(vec![f.response("", "alice/demo", "feature")]);
    assert_success(&f.checkout(port, "", "alice/demo", &f.local));
    assert_eq!(
        fs::read_to_string(module.join("private.txt")).unwrap(),
        "my ignored submodule data\n"
    );
    assert_eq!(git_output(&module, &["rev-parse", "HEAD"]), before_head);
    assert_eq!(
        git_output(
            &module,
            &["for-each-ref", "--format=%(refname) %(objectname)"]
        ),
        before_refs
    );
    assert_eq!(
        fs::read_to_string(fetch_head).unwrap(),
        "keep submodule fetch head\n"
    );
    assert_eq!(git_output(&f.local, &["branch", "--show-current"]), "pr-7");
    server.join().unwrap();
}

#[test]
fn refuses_submodule_changes_even_when_git_config_hides_them() {
    let f = Fixture::new();
    f.add_submodule();
    let module = f.local.join("module");
    fs::write(module.join("README.md"), "uncommitted submodule work\n").unwrap();
    fs::write(module.join("untracked.txt"), "untracked submodule work\n").unwrap();
    run_git(&f.local, &["config", "submodule.module.ignore", "all"]);
    assert_eq!(git_output(&f.local, &["status", "--porcelain=v1"]), "");
    let (port, server) = spawn_scripted_server(vec![f.response("", "alice/demo", "feature")]);
    f.refuse_unchanged(port, "", "alice/demo", "clean working tree");
    assert_eq!(
        fs::read_to_string(module.join("README.md")).unwrap(),
        "uncommitted submodule work\n"
    );
    assert_eq!(
        fs::read_to_string(module.join("untracked.txt")).unwrap(),
        "untracked submodule work\n"
    );
    server.join().unwrap();
}

#[test]
fn fetch_does_not_update_remote_tracking_refs_tags_or_fetch_head() {
    let f = Fixture::new();
    run_git(
        &f.local,
        &[
            "config",
            "remote.origin.fetch",
            "+refs/heads/*:refs/heads/*",
        ],
    );
    run_git(&f.source, &["tag", "remote-tag"]);
    run_git(&f.source, &["push", "origin", "remote-tag"]);
    fs::write(f.local.join(".git/FETCH_HEAD"), "keep fetch head\n").unwrap();
    let tracking = git_output(
        &f.local,
        &[
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            "refs/remotes",
        ],
    );
    let (port, server) = spawn_scripted_server(vec![f.response("", "alice/demo", "feature")]);
    assert_success(&f.checkout(port, "", "alice/demo", &f.local));
    assert_eq!(
        git_output(
            &f.local,
            &[
                "for-each-ref",
                "--format=%(refname) %(objectname)",
                "refs/remotes"
            ]
        ),
        tracking
    );
    assert_eq!(git_output(&f.local, &["tag", "--list"]), "");
    assert_eq!(
        fs::read_to_string(f.local.join(".git/FETCH_HEAD")).unwrap(),
        "keep fetch head\n"
    );
    assert_eq!(
        git_output(&f.local, &["rev-parse", "main"]),
        git_output(&f.source, &["rev-parse", "main"])
    );
    server.join().unwrap();
}

#[test]
fn lock_contention_refuses_before_local_changes() {
    let f = Fixture::new();
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(f.local.join(".git/gb-pr-checkout.lock"))
        .unwrap();
    fs2::FileExt::lock_exclusive(&file).unwrap();
    let (port, server) = spawn_scripted_server(vec![f.response("", "alice/demo", "feature")]);
    f.refuse_unchanged(port, "", "alice/demo", "Another gb PR checkout");
    server.join().unwrap();
}

#[test]
fn refuses_in_progress_operations_even_with_clean_index() {
    let f = Fixture::new();
    fs::create_dir(f.local.join(".git/rebase-merge")).unwrap();
    let (port, server) = spawn_scripted_server(vec![f.response("", "alice/demo", "feature")]);
    f.refuse_unchanged(port, "", "alice/demo", "in progress");
    server.join().unwrap();
}

#[test]
fn rejects_invalid_api_head_ref_before_fetching() {
    let f = Fixture::new();
    let (port, server) = spawn_scripted_server(vec![f.response(
        "",
        "alice/demo",
        "feature:refs/heads/main",
    )]);
    f.refuse_unchanged(port, "", "alice/demo", "check-ref-format failed");
    server.join().unwrap();
}

#[test]
fn namespaced_fetch_refs_distinguish_repositories_without_touching_legacy_refs() {
    let f = Fixture::new();
    run_git(&f.local, &["update-ref", "refs/gb/pr/7/head", "main"]);
    let legacy = git_output(&f.local, &["rev-parse", "refs/gb/pr/7/head"]);
    let (port, server) = spawn_scripted_server(vec![
        f.response("", "alice/demo", "feature"),
        f.response("", "alice/other", "feature"),
    ]);
    assert_success(&f.checkout(port, "", "alice/demo", &f.local));
    run_git(&f.local, &["checkout", "main"]);
    run_git(&f.local, &["branch", "-m", "pr-7", "keep-first-pr"]);
    assert_success(&f.checkout(port, "", "alice/other", &f.local));
    let refs = git_output(
        &f.local,
        &["for-each-ref", "--format=%(refname)", "refs/gb/pr-targets"],
    );
    assert_eq!(refs.lines().count(), 2);
    assert_eq!(
        git_output(&f.local, &["rev-parse", "refs/gb/pr/7/head"]),
        legacy
    );
    assert_eq!(
        git_output(&f.local, &["rev-parse", "keep-first-pr"]),
        git_output(&f.local, &["rev-parse", "pr-7"])
    );
    server.join().unwrap();
}

#[test]
fn detached_head_can_create_a_new_pr_branch() {
    let f = Fixture::new();
    run_git(&f.local, &["checkout", "--detach", "main"]);
    let (port, server) = spawn_scripted_server(vec![f.response("", "alice/demo", "feature")]);
    assert_success(&f.checkout(port, "", "alice/demo", &f.local));
    assert_eq!(git_output(&f.local, &["branch", "--show-current"]), "pr-7");
    server.join().unwrap();
}

#[test]
fn unwritable_git_config_fails_before_switching_branches() {
    let f = Fixture::new();
    fs::write(f.local.join(".git/config.lock"), "other config writer\n").unwrap();
    let (port, server) = spawn_scripted_server(vec![f.response("", "alice/demo", "feature")]);
    f.refuse_unchanged(port, "", "alice/demo", "git config failed");
    assert_eq!(
        fs::read_to_string(f.local.join(".git/config.lock")).unwrap(),
        "other config writer\n"
    );
    server.join().unwrap();
}

#[cfg(unix)]
#[test]
fn failing_post_checkout_hook_reports_completed_switch_and_retains_provenance() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let hook = f.local.join(".git/hooks/post-checkout");
    fs::write(
        &hook,
        "#!/bin/sh\necho 'injected post-checkout failure' >&2\nexit 1\n",
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o700)).unwrap();
    let main = git_output(&f.local, &["rev-parse", "main"]);
    let (port, server) = spawn_scripted_server(vec![
        f.response("", "alice/demo", "feature"),
        f.response("", "alice/demo", "feature"),
    ]);
    let output = f.checkout(port, "", "alice/demo", &f.local);
    assert_failure(&output);
    assert!(
        stderr(&output).contains("requested branch and commit are active"),
        "{}",
        stderr(&output)
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("✓"));
    assert_eq!(git_output(&f.local, &["rev-parse", "main"]), main);
    assert_eq!(git_output(&f.local, &["branch", "--show-current"]), "pr-7");
    fs::remove_file(hook).unwrap();
    assert_success(&f.checkout(port, "", "alice/demo", &f.local));
    server.join().unwrap();
}
