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
            .map(|label| label.name.clone())
            .collect();
        let mut progress = crate::operation::Progress::default();
        let removals: Vec<_> = self.remove.intersection(&current).collect();
        let steps: Vec<_> = removals
            .iter()
            .map(|name| {
                progress.plan(
                    format!("label.remove:{name}"),
                    crate::operation::Route::Rest,
                )
            })
            .collect();
        let additions: Vec<_> = self.add.difference(&current).cloned().collect();
        let add_step = (!additions.is_empty())
            .then(|| progress.plan("label.add", crate::operation::Route::Rest));
        for (name, step) in removals.into_iter().zip(steps) {
            progress
                .run(
                    step,
                    ctx.client
                        .remove_issue_label(&ctx.owner, &ctx.repo, number, name),
                    |_| async {
                        let actual = ctx
                            .client
                            .list_issue_labels(&ctx.owner, &ctx.repo, number)
                            .await?;
                        if actual.iter().any(|label| label.name == *name) {
                            return Err(GbError::Other(format!(
                                "Label {name:?} is still attached."
                            )));
                        }
                        Ok(())
                    },
                )
                .await?;
        }
        if let Some(step) = add_step {
            progress
                .run(
                    step,
                    ctx.client
                        .add_issue_labels(&ctx.owner, &ctx.repo, number, additions.clone()),
                    |_| async {
                        let actual = ctx
                            .client
                            .list_issue_labels(&ctx.owner, &ctx.repo, number)
                            .await?;
                        let actual: BTreeSet<_> =
                            actual.into_iter().map(|label| label.name.clone()).collect();
                        if !additions.iter().all(|name| actual.contains(name)) {
                            return Err(GbError::Other(
                                "Stored Issue labels do not confirm the additions.".into(),
                            ));
                        }
                        Ok(())
                    },
                )
                .await?;
        }
        Ok(())
    }
}
