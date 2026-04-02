pub mod actions;
pub mod plan;

use crate::config::schema::{GroupConfig, PermissionsConfig, TopicConfig};
use crate::provider::telegram::{GroupState, TopicState};
use crate::state::types::ResourceType;

use actions::{Action, FieldChange, ResourcePlan};

pub fn diff_group(
    group_name: &str,
    config: &GroupConfig,
    live: &GroupState,
    live_topics: &[TopicState],
) -> Vec<Action> {
    let mut actions = Vec::new();

    let group_key = format!("group.{group_name}");
    let mut group_changes = Vec::new();

    if let Some(ref desired_title) = config.title {
        if *desired_title != live.title {
            group_changes.push(FieldChange {
                field: "title".into(),
                old: Some(live.title.clone()),
                new: Some(desired_title.clone()),
            });
        }
    }

    if let Some(ref desired_desc) = config.description {
        if *desired_desc != live.description {
            group_changes.push(FieldChange {
                field: "description".into(),
                old: Some(live.description.clone()),
                new: Some(desired_desc.clone()),
            });
        }
    }

    if let Some(ref desired_perms) = config.permissions {
        if let Some(ref live_perms) = live.permissions {
            diff_permissions(desired_perms, live_perms, &mut group_changes);
        }
    }

    if !group_changes.is_empty() {
        actions.push(Action::Update(ResourcePlan {
            resource_key: group_key,
            resource_type: ResourceType::Group,
            topic_id: None,
            changes: group_changes,
        }));
    } else {
        actions.push(Action::NoOp);
    }

    diff_topics(group_name, &config.topic, live_topics, &mut actions);

    actions
}

fn diff_permissions(
    desired: &PermissionsConfig,
    live: &PermissionsConfig,
    changes: &mut Vec<FieldChange>,
) {
    macro_rules! check_perm {
        ($field:ident, $name:expr) => {
            if let Some(desired_val) = desired.$field {
                if let Some(live_val) = live.$field {
                    if desired_val != live_val {
                        changes.push(FieldChange {
                            field: format!("permissions.{}", $name),
                            old: Some(live_val.to_string()),
                            new: Some(desired_val.to_string()),
                        });
                    }
                }
            }
        };
    }

    check_perm!(send_messages, "send_messages");
    check_perm!(send_media, "send_media");
    check_perm!(send_stickers, "send_stickers");
    check_perm!(send_gifs, "send_gifs");
    check_perm!(send_polls, "send_polls");
    check_perm!(embed_links, "embed_links");
    check_perm!(invite_users, "invite_users");
    check_perm!(pin_messages, "pin_messages");
    check_perm!(change_info, "change_info");
}

fn diff_topics(
    group_name: &str,
    desired: &[TopicConfig],
    live: &[TopicState],
    actions: &mut Vec<Action>,
) {
    for topic_config in desired {
        let topic_key = format!("forum_topic.{group_name}.{}", topic_config.title);

        if let Some(live_topic) = live.iter().find(|t| t.title == topic_config.title) {
            let mut changes = Vec::new();

            if topic_config.closed != live_topic.closed {
                changes.push(FieldChange {
                    field: "closed".into(),
                    old: Some(live_topic.closed.to_string()),
                    new: Some(topic_config.closed.to_string()),
                });
            }

            if topic_config.icon_emoji_id != live_topic.icon_emoji_id {
                changes.push(FieldChange {
                    field: "icon_emoji_id".into(),
                    old: live_topic.icon_emoji_id.map(|v| v.to_string()),
                    new: topic_config.icon_emoji_id.map(|v| v.to_string()),
                });
            }

            if changes.is_empty() {
                actions.push(Action::NoOp);
            } else {
                actions.push(Action::Update(ResourcePlan {
                    resource_key: topic_key,
                    resource_type: ResourceType::ForumTopic,
                    topic_id: Some(live_topic.id),
                    changes,
                }));
            }
        } else {
            let mut changes = vec![FieldChange {
                field: "title".into(),
                old: None,
                new: Some(topic_config.title.clone()),
            }];

            if let Some(emoji_id) = topic_config.icon_emoji_id {
                changes.push(FieldChange {
                    field: "icon_emoji_id".into(),
                    old: None,
                    new: Some(emoji_id.to_string()),
                });
            }

            if topic_config.closed {
                changes.push(FieldChange {
                    field: "closed".into(),
                    old: None,
                    new: Some("true".into()),
                });
            }

            actions.push(Action::Create(ResourcePlan {
                resource_key: topic_key,
                resource_type: ResourceType::ForumTopic,
                topic_id: None,
                changes,
            }));
        }
    }

    // Topics in live but not in config — mark for deletion (but don't delete by default)
    for live_topic in live {
        if !desired.iter().any(|t| t.title == live_topic.title) {
            let topic_key = format!("forum_topic.{group_name}.{}", live_topic.title);
            actions.push(Action::Delete(ResourcePlan {
                resource_key: topic_key,
                resource_type: ResourceType::ForumTopic,
                topic_id: Some(live_topic.id),
                changes: vec![FieldChange {
                    field: "title".into(),
                    old: Some(live_topic.title.clone()),
                    new: None,
                }],
            }));
        }
    }
}
