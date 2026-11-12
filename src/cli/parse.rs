//! Error-format selection is lexical; values containing flag text are data.
use clap::{builder::TypedValueParser, error::ErrorKind};
use std::ffi::OsString;

pub(crate) fn json_errors_requested(args: &[OsString]) -> bool {
    args.iter()
        .skip(1)
        .take_while(|arg| *arg != "--")
        .any(|arg| arg == "--json-errors")
        || std::env::var_os("GB_JSON_ERRORS").is_some_and(|value| {
            clap::builder::BoolishValueParser::new()
                .parse_ref(&clap::Command::new("gb"), None, &value)
                .unwrap_or(false)
        })
}

pub(crate) fn cause(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::InvalidValue | ErrorKind::ValueValidation => "invalid_value",
        ErrorKind::UnknownArgument => "unknown_argument",
        ErrorKind::InvalidSubcommand => "invalid_subcommand",
        ErrorKind::MissingRequiredArgument => "missing_argument",
        ErrorKind::MissingSubcommand | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            "missing_subcommand"
        }
        ErrorKind::ArgumentConflict => "argument_conflict",
        ErrorKind::InvalidUtf8 => "invalid_utf8",
        ErrorKind::TooManyValues | ErrorKind::TooFewValues | ErrorKind::WrongNumberOfValues => {
            "invalid_value_count"
        }
        _ => "argument_error",
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;
    #[test]
    fn cli_values_do_not_consume_option_like_tokens_without_an_equals_or_terminator() {
        // If a command begins accepting arbitrary hyphenated/trailing values,
        // error-format selection must use that new grammar rather than treating
        // a standalone --json-errors token as a flag.
        fn inspect(command: &clap::Command) {
            assert!(
                !command.is_allow_external_subcommands_set(),
                "{}",
                command.get_name()
            );
            for arg in command.get_arguments() {
                assert!(
                    !arg.is_allow_hyphen_values_set(),
                    "{}: {}",
                    command.get_name(),
                    arg.get_id()
                );
                assert!(
                    !arg.is_trailing_var_arg_set(),
                    "{}: {}",
                    command.get_name(),
                    arg.get_id()
                );
            }
            for child in command.get_subcommands() {
                inspect(child);
            }
        }
        inspect(&super::super::Cli::command());
    }
}
