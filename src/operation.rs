//! Mutation progress records confirmed effects, never an assumed rollback.
use std::future::Future;

use serde::Serialize;

use crate::error::{GbError, Result};

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    Rest,
    Web,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Succeeded,
    Failed,
    Unverified,
    NotAttempted,
}

#[derive(Clone, Debug, Serialize)]
pub struct Operation {
    pub operation: String,
    pub route: Route,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
}

#[derive(Default)]
pub struct Progress {
    operations: Vec<Operation>,
}

impl Progress {
    pub fn plan(&mut self, operation: impl Into<String>, route: Route) -> usize {
        let index = self.operations.len();
        self.operations.push(Operation {
            operation: operation.into(),
            route,
            status: Status::NotAttempted,
            actor: None,
        });
        index
    }

    pub fn actor(&mut self, actor: &str) {
        for operation in &mut self.operations {
            if matches!(operation.route, Route::Web) {
                operation.actor = Some(actor.into());
            }
        }
    }

    pub fn route(&mut self, index: usize, route: Route) {
        self.operations[index].route = route;
    }

    /// A successful transport is only a submission. Verification must establish
    /// the intended stored result before a step can be marked succeeded.
    pub async fn run<T, U, F, V, VF>(&mut self, index: usize, submit: F, verify: V) -> Result<U>
    where
        F: Future<Output = Result<T>>,
        V: FnOnce(T) -> VF,
        VF: Future<Output = Result<U>>,
    {
        self.operations[index].status = Status::Unverified;
        let submitted = match submit.await {
            Ok(value) => value,
            Err(err) => {
                // Only a definite rejection establishes failure. Network, 5xx,
                // parse errors and readback failures may follow an applied write.
                if matches!(
                    err,
                    GbError::Api {
                        status: 400..=499,
                        ..
                    } | GbError::Auth(_)
                ) {
                    self.operations[index].status = Status::Failed;
                }
                return Err(self.error(err));
            }
        };
        match verify(submitted).await {
            Ok(value) => {
                self.operations[index].status = Status::Succeeded;
                Ok(value)
            }
            Err(err) => Err(self.error(err)),
        }
    }

    pub fn error(&self, error: GbError) -> GbError {
        let mut operations = self.operations.clone();
        let source = match error {
            GbError::Operation {
                source,
                operations: nested,
            } => {
                operations.extend(nested);
                source
            }
            source => Box::new(source),
        };
        GbError::Operation { source, operations }
    }
}

pub fn describe(operations: &[Operation]) -> String {
    operations
        .iter()
        .map(|op| {
            format!(
                "{}={}",
                op.operation,
                match op.status {
                    Status::Succeeded => "succeeded",
                    Status::Failed => "failed",
                    Status::Unverified => "unverified",
                    Status::NotAttempted => "not_attempted",
                }
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}
