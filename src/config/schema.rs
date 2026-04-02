use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub provider: ProviderConfig,
    #[serde(default)]
    pub group: BTreeMap<String, GroupConfig>,
}

#[derive(Debug, Deserialize)]
pub struct ProviderConfig {
    pub api_id: i32,
    pub api_hash: String,
    #[serde(default = "default_session_file")]
    pub session_file: String,
}

fn default_session_file() -> String {
    "tgctl.session".to_string()
}

#[derive(Debug, Deserialize)]
pub struct GroupConfig {
    /// "@username" or numeric chat ID
    pub chat: String,
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub permissions: Option<PermissionsConfig>,
    #[serde(default)]
    pub topic: Vec<TopicConfig>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PermissionsConfig {
    pub send_messages: Option<bool>,
    pub send_media: Option<bool>,
    pub send_stickers: Option<bool>,
    pub send_gifs: Option<bool>,
    pub send_polls: Option<bool>,
    pub embed_links: Option<bool>,
    pub invite_users: Option<bool>,
    pub pin_messages: Option<bool>,
    pub change_info: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TopicConfig {
    pub title: String,
    pub icon_emoji_id: Option<i64>,
    #[serde(default)]
    pub closed: bool,
}
