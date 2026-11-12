//! Explicit UTF-8 input and terminal-only prompts shared by commands.
use crate::error::{GbError, Result};
use std::{io::Read, path::PathBuf};

pub(crate) fn interactive() -> bool {
    crate::cancel::can_prompt()
}

pub(crate) fn required(option: &str) -> GbError {
    GbError::Context {
        source: Box::new(GbError::InputRequired),
        context: format!("Supply {option}"),
    }
}

pub(crate) async fn body(body: Option<String>, file: Option<PathBuf>) -> Result<Option<String>> {
    match (body, file) {
        (Some(_), Some(_)) => Err(GbError::Other("Choose --body or --body-file.".into())),
        (None, Some(file)) => {
            crate::cancel::input(move || -> Result<_> {
                let text = if file == std::path::Path::new("-") {
                    let mut text = String::new();
                    std::io::stdin().read_to_string(&mut text)?;
                    text
                } else {
                    std::fs::read_to_string(file)?
                };
                Ok(Some(text))
            })
            .await
        }
        (body, None) => Ok(body),
    }
}

pub(crate) async fn title(title: Option<String>) -> Result<String> {
    let title = match title {
        Some(title) => title,
        None if !interactive() => return Err(required("--title")),
        None => {
            crate::cancel::prompt(|| {
                dialoguer::Input::<String>::new()
                    .with_prompt("Title")
                    .interact_text()
            })
            .await?
        }
    };
    if title.trim().is_empty() {
        return Err(GbError::Other("Title must not be blank.".into()));
    }
    Ok(title)
}

pub(crate) async fn optional_body(body: Option<String>) -> Result<Option<String>> {
    if body.is_some() || !interactive() {
        return Ok(body);
    }
    let text: String = crate::cancel::prompt(|| {
        dialoguer::Input::new()
            .with_prompt("Body (optional)")
            .allow_empty(true)
            .interact_text()
    })
    .await?;
    Ok((!text.is_empty()).then_some(text))
}

pub(crate) async fn comment(body: Option<String>) -> Result<String> {
    let text = match body {
        Some(text) => text,
        None if !interactive() => return Err(required("--body or --body-file (use - for stdin)")),
        None => {
            crate::cancel::prompt(|| {
                dialoguer::Input::<String>::new()
                    .with_prompt("Comment body")
                    .interact_text()
            })
            .await?
        }
    };
    if text.trim().is_empty() {
        return Err(GbError::Other("Comment body must not be blank.".into()));
    }
    Ok(text)
}

pub(crate) async fn token_stdin() -> Result<String> {
    crate::cancel::input(|| -> Result<String> {
        // A token is a single line; never include rejected input in diagnostics.
        let mut text = String::new();
        std::io::stdin()
            .take(65537)
            .read_to_string(&mut text)
            .map_err(|_| GbError::Other("Could not read a UTF-8 token from stdin.".into()))?;
        let token = text
            .strip_suffix("\r\n")
            .or_else(|| text.strip_suffix('\n'))
            .unwrap_or(&text);
        if text.len() > 65536
            || token.is_empty()
            || token.chars().any(char::is_whitespace)
            || token.chars().any(char::is_control)
        {
            return Err(GbError::Other(
                "Stdin must contain one nonempty token, optionally followed by a line ending."
                    .into(),
            ));
        }
        Ok(token.to_owned())
    })
    .await
}
