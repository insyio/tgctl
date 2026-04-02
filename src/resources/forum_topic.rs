use serde_json::json;

use crate::provider::telegram::TopicState;
use crate::state::types::{ResourceType, StateResource};

pub fn topic_to_state(group_name: &str, topic: &TopicState) -> StateResource {
    StateResource {
        resource_type: ResourceType::ForumTopic,
        name: topic.title.clone(),
        chat_id: None,
        topic_id: Some(topic.id),
        parent_group: Some(group_name.to_string()),
        attributes: json!({
            "title": topic.title,
            "icon_emoji_id": topic.icon_emoji_id,
            "closed": topic.closed,
        }),
    }
}
