use clap::Args;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::error::Result;

#[derive(Args)]
pub(crate) struct JsonOutput {
    /// Output as JSON
    #[arg(long)]
    json: bool,
    /// Output only these JSON fields (comma-separated, e.g. title,user.login)
    #[arg(long, value_name = "FIELDS", value_delimiter = ',', value_parser = field_path)]
    json_fields: Vec<String>,
}

impl JsonOutput {
    pub(super) fn enabled(&self) -> bool {
        self.json || !self.json_fields.is_empty()
    }

    pub(super) fn page(&self, value: &impl Serialize, no_pager: bool) -> Result<()> {
        if self.json_fields.is_empty() {
            return crate::output::page_json(value, no_pager);
        }

        self.page_value(serde_json::to_value(value)?, no_pager)
    }

    pub(super) fn page_value(&self, mut value: Value, no_pager: bool) -> Result<()> {
        if self.json_fields.is_empty() {
            return crate::output::page_json(&value, no_pager);
        }

        let paths: Vec<&str> = self.json_fields.iter().map(String::as_str).collect();
        match &mut value {
            Value::Array(items) => {
                for item in items {
                    project(item, &paths);
                }
            }
            _ => project(&mut value, &paths),
        }

        crate::output::page_json(&value, no_pager)
    }
}

fn field_path(input: &str) -> std::result::Result<String, &'static str> {
    let path = input.trim();

    if path.split('.').any(str::is_empty)
        || path.chars().any(|ch| ch.is_whitespace() || ch.is_control())
    {
        return Err("Expected a JSON field name or a dot-separated field path");
    }

    Ok(path.to_owned())
}

fn project(value: &mut Value, paths: &[&str]) {
    if let Value::Object(object) = value {
        retain_fields(object, paths);
    }
}

fn retain_fields(object: &mut Map<String, Value>, paths: &[&str]) {
    object.retain(|key, value| {
        if key.contains('.') {
            return false;
        }
        if paths.contains(&key.as_str()) {
            return true;
        }

        let Value::Object(nested) = value else {
            return false;
        };
        let children: Vec<_> = paths
            .iter()
            .filter_map(|path| {
                let (parent, child) = path.split_once('.')?;
                (parent == key).then_some(child)
            })
            .collect();
        if children.is_empty() {
            return false;
        }

        retain_fields(nested, &children);
        !nested.is_empty()
    });
}
