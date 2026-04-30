mod auth;
mod context;
mod edit_values;
mod repo_resolution;
mod web;

pub use auth::{create_client, resolve_host_config, resolve_hostname, validate_selected_profile};
pub use context::{HostContext, RepoContext};
pub use edit_values::{
    merge_named_values, normalize_edit_state, normalize_list_state, normalize_str_vec,
};
pub use repo_resolution::{parse_owner_repo, resolve_repo};
pub use web::{create_web_session, update_issue_assignees_via_web};

pub(crate) use repo_resolution::parse_git_url;
