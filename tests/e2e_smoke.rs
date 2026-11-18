#[path = "support/gb_cmd.rs"]
mod gb_cmd;

#[path = "e2e_smoke/support.rs"]
mod support;

#[cfg(unix)]
#[path = "support/editor.rs"]
mod editor;
#[cfg(unix)]
#[test]
fn editor_fixture() {
    editor::fixture();
}

#[path = "e2e_smoke/auth_config.rs"]
mod auth_config;
#[path = "e2e_smoke/issue.rs"]
mod issue;
#[path = "e2e_smoke/pr.rs"]
mod pr;
#[path = "e2e_smoke/repo_label_milestone.rs"]
mod repo_label_milestone;

#[path = "e2e_smoke/api_contract.rs"]
mod api_contract;

#[path = "e2e_smoke/pr_snapshot.rs"]
mod pr_snapshot;

#[path = "e2e_smoke/review_merge.rs"]
mod review_merge;
