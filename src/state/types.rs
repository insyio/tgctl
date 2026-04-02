use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct StateFile {
    pub version: u32,
    pub serial: u64,
    pub resources: BTreeMap<String, StateResource>,
}

impl StateFile {
    pub fn new() -> Self {
        Self {
            version: 1,
            serial: 0,
            resources: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateResource {
    #[serde(rename = "type")]
    pub resource_type: ResourceType,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_group: Option<String>,
    pub attributes: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    Group,
    ForumTopic,
}
