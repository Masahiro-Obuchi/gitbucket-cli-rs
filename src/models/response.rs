//! Keep server JSON immutable alongside a validated projection used by commands.
use std::ops::Deref;

use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

#[derive(Debug, Clone)]
pub struct ApiResponse<T> {
    fields: T,
    json: Map<String, Value>,
}

impl<T> Deref for ApiResponse<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.fields
    }
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for ApiResponse<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let json = Map::<String, Value>::deserialize(deserializer)?;
        let fields = serde_json::from_value(Value::Object(json.clone()))
            .map_err(serde::de::Error::custom)?;
        Ok(Self { fields, json })
    }
}

impl<T> Serialize for ApiResponse<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.json.serialize(serializer)
    }
}

impl<T: DeserializeOwned> ApiResponse<T> {
    /// An explicit transformation must update and revalidate both views.
    pub fn with_field(mut self, name: &str, value: Value) -> serde_json::Result<Self> {
        self.json.insert(name.into(), value);
        serde_json::from_value(Value::Object(self.json))
    }
}

impl<T: DeserializeOwned + Serialize> ApiResponse<T> {
    /// Canonical PR details own known fields, including absent fields. Preserve
    /// extra collection metadata without restoring unverified summary head data.
    pub fn with_unknown_from(mut self, summary: &Self) -> serde_json::Result<Self> {
        let projection = serde_json::to_value(&self.fields)?;
        let known = projection.as_object().ok_or_else(|| {
            <serde_json::Error as serde::ser::Error>::custom("Expected object response fields")
        })?;
        for (name, value) in &summary.json {
            if !known.contains_key(name) {
                self.json
                    .entry(name.clone())
                    .or_insert_with(|| value.clone());
            }
        }
        serde_json::from_value(Value::Object(self.json))
    }
}
