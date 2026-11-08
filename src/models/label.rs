use serde::{Deserialize, Serialize};

pub type Label = super::response::ApiResponse<LabelFields>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LabelFields {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateLabel {
    pub name: String,
    pub color: String,
}

/// GitBucket 4.44 requires name and color even for a single-field edit.
#[derive(Debug, Serialize)]
pub struct UpdateLabel {
    pub name: String,
    pub color: String,
    // Preserve a description exposed by another implementation; the CLI does
    // not promise unsupported description mutations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AddIssueLabels {
    pub labels: Vec<String>,
}
