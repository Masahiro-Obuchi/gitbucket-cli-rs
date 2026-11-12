//! One error record on stderr; diagnostics do not echo raw argument values.
use crate::{api, error, operation};
use serde::Serialize;
use std::io::Write;

#[derive(Serialize)]
struct ErrorOutput {
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    child_exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outcome: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    operations: Option<Vec<operation::Operation>>,
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cause: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_code: Option<i32>,
}

pub(crate) fn report_error(error: &error::GbError, json: bool, token: Option<&str>) {
    if !json {
        let _ = writeln!(
            std::io::stderr().lock(),
            "Error: {}",
            super::text::human(&error.to_string())
        );
        return;
    }
    let secrets = api::transport::Secrets::new(&[token.unwrap_or_default()]);
    print_error_record(runtime_error(error, &secrets));
}

fn runtime_error(error: &error::GbError, secrets: &api::transport::Secrets) -> ErrorBody {
    let message = if error.code() == "toml_deserialization_error" {
        "Invalid configuration TOML. Inspect the local configuration file.".into()
    } else {
        secrets.clean(&error.to_string())
    };
    let operations = error.operations().map(|operations| {
        operations
            .iter()
            .cloned()
            .map(|mut operation| {
                operation.operation = secrets.clean(&operation.operation);
                operation.actor = operation.actor.map(|actor| secrets.clean(&actor));
                operation
            })
            .collect()
    });
    ErrorBody {
        child_exit_code: error.child_exit_code(),
        outcome: error.outcome(),
        operations,
        code: error.code(),
        message,
        cause: error.cause_code(),
        status: error.status(),
        exit_code: Some(1),
    }
}

fn print_error_record(error: ErrorBody) {
    let output = ErrorOutput { error };
    match serde_json::to_string(&output) {
        Ok(json) => {
            let _ = writeln!(std::io::stderr().lock(), "{json}");
        }
        Err(_) => {
            let _ = writeln!(
                std::io::stderr().lock(),
                "{{\"error\":{{\"code\":\"error_serialization_failed\",\"exit_code\":1}}}}"
            );
        }
    }
}

pub(crate) fn argument_error(kind: &str, cause: &'static str) {
    print_error_record(ErrorBody {
        code: "invalid_arguments",
        message: format!("Invalid CLI arguments: {kind}. Run `gb <command> --help` for usage."),
        cause: Some(cause),
        exit_code: Some(2),
        outcome: None,
        operations: None,
        status: None,
        child_exit_code: None,
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn partial_outcomes_keep_typed_metadata_while_redacting_all_diagnostic_strings() {
        use crate::{
            error::GbError,
            operation::{Operation, Route, Status},
        };
        let secret = "private-diagnostic-value";
        let error = GbError::Operation {
            source: Box::new(GbError::Api {
                status: 502,
                message: secret.into(),
            }),
            operations: vec![Operation {
                operation: format!("label.add:{secret}"),
                actor: Some(secret.into()),
                route: Route::Web,
                status: Status::Unverified,
            }],
        };
        let value = serde_json::to_value(super::runtime_error(
            &error,
            &crate::api::transport::Secrets::new(&[secret]),
        ))
        .unwrap();
        assert_eq!(value["code"], "api_error");
        assert_eq!(value["status"], 502);
        assert_eq!(value["exit_code"], 1);
        assert_eq!(value["outcome"], "unverified");
        assert_eq!(value["operations"][0]["route"], "web");
        assert_eq!(value["operations"][0]["status"], "unverified");
        assert!(!value.to_string().contains(secret));
    }
}
