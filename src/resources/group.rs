use serde_json::json;

use crate::provider::telegram::GroupState;
use crate::state::types::{ResourceType, StateResource};

pub fn group_to_state(name: &str, live: &GroupState) -> StateResource {
    let perms = live.permissions.as_ref().map(|p| {
        json!({
            "send_messages": p.send_messages,
            "send_media": p.send_media,
            "send_stickers": p.send_stickers,
            "send_gifs": p.send_gifs,
            "send_polls": p.send_polls,
            "embed_links": p.embed_links,
            "invite_users": p.invite_users,
            "pin_messages": p.pin_messages,
            "change_info": p.change_info,
        })
    });

    StateResource {
        resource_type: ResourceType::Group,
        name: name.to_string(),
        chat_id: Some(live.chat_id),
        topic_id: None,
        parent_group: None,
        attributes: json!({
            "title": live.title,
            "description": live.description,
            "permissions": perms,
        }),
    }
}
