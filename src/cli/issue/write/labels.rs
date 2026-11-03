//! Issue label intent is applied as deltas, never as a full-set replacement.
use std::collections::BTreeSet;

use crate::api::label::validate_label_segment;
use crate::cli::common::RepoContext;
use crate::error::{GbError, Result};

pub(super) struct LabelChanges {
    add: BTreeSet<String>,
    remove: BTreeSet<String>,
}

impl LabelChanges {
    pub(super) fn new(add: Vec<String>, remove: Vec<String>) -> Result<Self> {
        for name in add.iter().chain(&remove) {
            validate_label_segment(name)?;
        }
        let add: BTreeSet<_> = add.into_iter().collect();
        let remove = remove
            .into_iter()
            .filter(|name| !add.contains(name))
            .collect();
        Ok(Self { add, remove })
    }

    pub(super) fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty()
    }

    /// Fail before a metadata mutation if an addition would implicitly create a
    /// repository label (a non-GitHub behavior of the pinned server's add route).
    pub(super) async fn preflight(&self, ctx: &RepoContext) -> Result<()> {
        for name in &self.add {
            let label = ctx.client.get_label(&ctx.owner, &ctx.repo, name).await.map_err(|err| match err {
                GbError::Api { status, message } => GbError::Api { status, message: format!("Checking requested label {name:?} before edits: {message}. No Issue changes were applied; additions require an existing repository label.") },
                other => other,
            })?;
            if label.name != *name {
                return Err(GbError::Other(
                    "Issue label lookup returned another name. No changes were applied.".into(),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn verify(&self, labels: &[crate::models::label::Label]) -> Result<()> {
        let names: BTreeSet<_> = labels.iter().map(|label| label.name.clone()).collect();
        if !self.add.is_subset(&names) || !self.remove.is_disjoint(&names) {
            return Err(GbError::Other("Stored Issue labels do not match the requested additions/removals. Inspect the Issue before retrying; changes may already be applied.".into()));
        }
        Ok(())
    }

    pub(super) async fn apply(&self, ctx: &RepoContext, number: u64) -> Result<()> {
        let current: BTreeSet<_> = ctx
            .client
            .list_issue_labels(&ctx.owner, &ctx.repo, number)
            .await?
            .into_iter()
            .map(|label| label.name)
            .collect();
        let mut completed = Vec::new();
        for name in self.remove.intersection(&current) {
            ctx.client
                .remove_issue_label(&ctx.owner, &ctx.repo, number, name)
                .await
                .map_err(|err| partial(err, &completed))?;
            completed.push(format!("removed {name:?}"));
        }
        let additions: Vec<_> = self.add.difference(&current).cloned().collect();
        if !additions.is_empty() {
            ctx.client
                .add_issue_labels(&ctx.owner, &ctx.repo, number, additions.clone())
                .await
                .map_err(|err| partial(err, &completed))?;
            completed.push(format!("submitted additions {additions:?}"));
        }
        if completed.is_empty() {
            return Ok(());
        }
        let actual: BTreeSet<_> = ctx
            .client
            .list_issue_labels(&ctx.owner, &ctx.repo, number)
            .await
            .map_err(|err| partial(err, &completed))?
            .into_iter()
            .map(|label| label.name)
            .collect();
        if !self.add.is_subset(&actual) || !self.remove.is_disjoint(&actual) {
            return Err(partial(
                GbError::Other(
                    "Stored Issue labels do not match the requested additions/removals.".into(),
                ),
                &completed,
            ));
        }
        Ok(())
    }
}

fn partial(err: GbError, completed: &[String]) -> GbError {
    GbError::Other(format!("Issue label update failed: {err}. Completed label steps: {}. Inspect the Issue before retrying; an in-flight request may already be applied.", if completed.is_empty() { "none confirmed".into() } else { completed.join(", ") }))
}
