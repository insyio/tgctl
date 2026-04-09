use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use grammers_client::{Client, SenderPool};
use grammers_session::storages::SqliteSession;
use grammers_tl_types as tl;

use crate::config::schema::{PermissionsConfig, ProviderConfig};
use crate::diff::actions::{Action, FieldChange, ResourcePlan};
use crate::error::ProviderError;
use crate::state::types::ResourceType;

use super::auth;

pub struct TelegramProvider {
    pub client: Client,
}

impl TelegramProvider {
    pub async fn connect(config: &ProviderConfig) -> anyhow::Result<Self> {
        let session = SqliteSession::open(&config.session_file)
            .await
            .context("Failed to open session file")?;
        let session = Arc::new(session);

        let pool = SenderPool::new(session, config.api_id);
        tokio::spawn(pool.runner.run());

        let client = Client::new(pool.handle);

        auth::authenticate(&client, &config.api_hash)
            .await
            .context("Authentication failed")?;

        Ok(Self { client })
    }

    fn make_input_peer(channel: &tl::types::InputChannel) -> tl::enums::InputPeer {
        tl::enums::InputPeer::Channel(tl::types::InputPeerChannel {
            channel_id: channel.channel_id,
            access_hash: channel.access_hash,
        })
    }

    pub async fn resolve_chat(
        &self,
        chat: &str,
    ) -> Result<tl::types::InputChannel, ProviderError> {
        // Try as numeric ID first (e.g. "-1001234567890" or "1234567890")
        let stripped = chat.strip_prefix("-100").unwrap_or(chat);
        if let Ok(channel_id) = stripped.parse::<i64>() {
            return self.resolve_chat_by_dialogs(channel_id).await;
        }

        // Try as @username
        let username = chat.strip_prefix('@').unwrap_or(chat);

        let resolved = self
            .client
            .invoke(&tl::functions::contacts::ResolveUsername {
                username: username.to_string(),
                referer: None,
            })
            .await;

        match resolved {
            Ok(tl::enums::contacts::ResolvedPeer::Peer(resolved)) => {
                for c in &resolved.chats {
                    if let tl::enums::Chat::Channel(channel) = c {
                        return Ok(tl::types::InputChannel {
                            channel_id: channel.id,
                            access_hash: channel.access_hash.unwrap_or(0),
                        });
                    }
                }
                Err(ProviderError::ChatNotFound(format!(
                    "{chat}: resolved but no channel found"
                )))
            }
            Err(e) => Err(ProviderError::ChatNotFound(format!("{chat}: {e}"))),
        }
    }

    async fn resolve_chat_by_dialogs(
        &self,
        channel_id: i64,
    ) -> Result<tl::types::InputChannel, ProviderError> {
        // Fetch user's dialogs to find the channel with its access_hash
        let dialogs = self
            .client
            .invoke(&tl::functions::messages::GetDialogs {
                exclude_pinned: false,
                folder_id: None,
                offset_date: 0,
                offset_id: 0,
                offset_peer: tl::enums::InputPeer::Empty,
                limit: 100,
                hash: 0,
            })
            .await
            .map_err(|e| ProviderError::ChatNotFound(format!("Failed to fetch dialogs: {e}")))?;

        let chats = match &dialogs {
            tl::enums::messages::Dialogs::Dialogs(d) => &d.chats,
            tl::enums::messages::Dialogs::Slice(d) => &d.chats,
            tl::enums::messages::Dialogs::NotModified(_) => {
                return Err(ProviderError::ChatNotFound(
                    "Got NotModified response for dialogs".into(),
                ));
            }
        };

        for c in chats {
            if let tl::enums::Chat::Channel(channel) = c {
                if channel.id == channel_id {
                    return Ok(tl::types::InputChannel {
                        channel_id: channel.id,
                        access_hash: channel.access_hash.unwrap_or(0),
                    });
                }
            }
        }

        Err(ProviderError::ChatNotFound(format!(
            "Channel {channel_id} not found in dialogs. Make sure you are a member of this group."
        )))
    }

    pub async fn fetch_group(
        &self,
        channel: &tl::types::InputChannel,
    ) -> Result<GroupState, ProviderError> {
        let full = self
            .client
            .invoke(&tl::functions::channels::GetFullChannel {
                channel: tl::enums::InputChannel::Channel(channel.clone()),
            })
            .await?;

        let tl::enums::messages::ChatFull::Full(full) = full;

        // Extract channel info from chats list
        for c in &full.chats {
            if let tl::enums::Chat::Channel(ch) = c {
                let title = ch.title.clone();
                let chat_id = ch.id;

                let permissions = ch.default_banned_rights.as_ref().map(|br| {
                    let tl::enums::ChatBannedRights::Rights(rights) = br;
                    PermissionsConfig {
                        send_messages: Some(!rights.send_messages),
                        send_media: Some(!rights.send_media),
                        send_stickers: Some(!rights.send_stickers),
                        send_gifs: Some(!rights.send_gifs),
                        send_polls: Some(!rights.send_polls),
                        embed_links: Some(!rights.embed_links),
                        invite_users: Some(!rights.invite_users),
                        pin_messages: Some(!rights.pin_messages),
                        change_info: Some(!rights.change_info),
                    }
                });

                // Get description from full_chat
                let description = match &full.full_chat {
                    tl::enums::ChatFull::ChannelFull(cf) => cf.about.clone(),
                    tl::enums::ChatFull::Full(cf) => cf.about.clone(),
                };

                return Ok(GroupState {
                    chat_id,
                    title,
                    description,
                    permissions,
                });
            }
        }

        Err(ProviderError::ChatNotFound(
            "channel not found in response".into(),
        ))
    }

    pub async fn fetch_topics(
        &self,
        channel: &tl::types::InputChannel,
    ) -> Result<Vec<TopicState>, ProviderError> {
        let peer = Self::make_input_peer(channel);

        let result = self
            .client
            .invoke(&tl::functions::messages::GetForumTopics {
                peer,
                q: None,
                offset_date: 0,
                offset_id: 0,
                offset_topic: 0,
                limit: 100,
            })
            .await?;

        let mut topics = Vec::new();

        let tl::enums::messages::ForumTopics::Topics(forum_topics) = result;
        for topic in &forum_topics.topics {
            if let tl::enums::ForumTopic::Topic(t) = topic {
                topics.push(TopicState {
                    id: t.id,
                    title: t.title.clone(),
                    icon_emoji_id: t.icon_emoji_id,
                    closed: t.closed,
                });
            }
        }

        Ok(topics)
    }

    pub async fn apply_action(
        &self,
        action: &Action,
        channel: &tl::types::InputChannel,
    ) -> Result<(), ProviderError> {
        match action {
            Action::Create(plan) => self.apply_create(plan, channel).await,
            Action::Update(plan) => self.apply_update(plan, channel).await,
            Action::Delete(plan) => self.apply_delete(plan, channel).await,
            Action::NoOp => Ok(()),
        }
    }

    async fn apply_create(
        &self,
        plan: &ResourcePlan,
        channel: &tl::types::InputChannel,
    ) -> Result<(), ProviderError> {
        match plan.resource_type {
            ResourceType::ForumTopic => {
                let title = plan
                    .changes
                    .iter()
                    .find(|c| c.field == "title")
                    .and_then(|c| c.new.as_deref())
                    .unwrap_or("Untitled");

                let icon_emoji_id = plan
                    .changes
                    .iter()
                    .find(|c| c.field == "icon_emoji_id")
                    .and_then(|c| c.new.as_deref())
                    .and_then(|v| v.parse::<i64>().ok());

                let peer = Self::make_input_peer(channel);

                self.client
                    .invoke(&tl::functions::messages::CreateForumTopic {
                        title_missing: false,
                        peer,
                        title: title.to_string(),
                        icon_color: None,
                        icon_emoji_id,
                        random_id: rand_id(),
                        send_as: None,
                    })
                    .await?;

                Ok(())
            }
            ResourceType::Group => Err(ProviderError::PermissionDenied(
                "Cannot create groups — only manage existing ones".into(),
            )),
        }
    }

    async fn apply_update(
        &self,
        plan: &ResourcePlan,
        channel: &tl::types::InputChannel,
    ) -> Result<(), ProviderError> {
        let peer = Self::make_input_peer(channel);

        match plan.resource_type {
            ResourceType::Group => {
                for change in &plan.changes {
                    match change.field.as_str() {
                        "title" => {
                            if let Some(ref new_title) = change.new {
                                self.client
                                    .invoke(&tl::functions::channels::EditTitle {
                                        channel: tl::enums::InputChannel::Channel(
                                            channel.clone(),
                                        ),
                                        title: new_title.clone(),
                                    })
                                    .await?;
                            }
                        }
                        "description" => {
                            if let Some(ref new_desc) = change.new {
                                self.client
                                    .invoke(&tl::functions::messages::EditChatAbout {
                                        peer: peer.clone(),
                                        about: new_desc.clone(),
                                    })
                                    .await?;
                            }
                        }
                        _ => {} // permissions handled below
                    }
                }

                let has_perm_changes = plan
                    .changes
                    .iter()
                    .any(|c| c.field.starts_with("permissions."));

                if has_perm_changes {
                    self.apply_permissions(channel, &plan.changes).await?;
                }

                Ok(())
            }
            ResourceType::ForumTopic => {
                let topic_id = plan
                    .topic_id
                    .ok_or_else(|| ProviderError::Auth("Missing topic_id for update".into()))?;

                let title_change = plan.changes.iter().find(|c| c.field == "title");
                let closed_change = plan.changes.iter().find(|c| c.field == "closed");
                let emoji_change = plan.changes.iter().find(|c| c.field == "icon_emoji_id");

                let title = title_change.and_then(|c| c.new.clone());
                let icon_emoji_id = emoji_change
                    .and_then(|c| c.new.as_deref())
                    .and_then(|v| v.parse::<i64>().ok());
                let closed = closed_change
                    .and_then(|c| c.new.as_deref())
                    .and_then(|v| v.parse().ok());

                if title.is_some() || closed.is_some() || icon_emoji_id.is_some() {
                    self.client
                        .invoke(&tl::functions::messages::EditForumTopic {
                            peer,
                            topic_id,
                            title,
                            icon_emoji_id,
                            closed,
                            hidden: None,
                        })
                        .await?;
                }

                Ok(())
            }
        }
    }

    async fn apply_delete(
        &self,
        plan: &ResourcePlan,
        channel: &tl::types::InputChannel,
    ) -> Result<(), ProviderError> {
        match plan.resource_type {
            ResourceType::ForumTopic => {
                let topic_id = plan
                    .topic_id
                    .ok_or_else(|| ProviderError::Auth("Missing topic_id for delete".into()))?;

                let peer = Self::make_input_peer(channel);

                self.client
                    .invoke(&tl::functions::messages::DeleteTopicHistory {
                        peer,
                        top_msg_id: topic_id,
                    })
                    .await?;

                Ok(())
            }
            ResourceType::Group => Err(ProviderError::PermissionDenied(
                "Cannot delete groups".into(),
            )),
        }
    }

    async fn apply_permissions(
        &self,
        channel: &tl::types::InputChannel,
        changes: &[FieldChange],
    ) -> Result<(), ProviderError> {
        let get_bool = |field: &str| -> bool {
            changes
                .iter()
                .find(|c| c.field == field)
                .and_then(|c| c.new.as_deref())
                .and_then(|v| v.parse::<bool>().ok())
                .unwrap_or(true)
        };

        // Banned rights use inverted logic: true = banned
        let banned_rights =
            tl::enums::ChatBannedRights::Rights(tl::types::ChatBannedRights {
                until_date: 0,
                view_messages: false,
                send_messages: !get_bool("permissions.send_messages"),
                send_media: !get_bool("permissions.send_media"),
                send_stickers: !get_bool("permissions.send_stickers"),
                send_gifs: !get_bool("permissions.send_gifs"),
                send_games: false,
                send_inline: false,
                send_polls: !get_bool("permissions.send_polls"),
                change_info: !get_bool("permissions.change_info"),
                invite_users: !get_bool("permissions.invite_users"),
                pin_messages: !get_bool("permissions.pin_messages"),
                manage_topics: false,
                send_photos: false,
                send_videos: false,
                send_roundvideos: false,
                send_audios: false,
                send_voices: false,
                send_docs: false,
                send_plain: false,
                embed_links: !get_bool("permissions.embed_links"),
            });

        let peer = Self::make_input_peer(channel);

        self.client
            .invoke(&tl::functions::messages::EditChatDefaultBannedRights {
                peer,
                banned_rights,
            })
            .await?;

        Ok(())
    }

    pub async fn fetch_emoji_pack(
        &self,
        short_name: &str,
    ) -> Result<EmojiPackInfo, ProviderError> {
        let result = self
            .client
            .invoke(&tl::functions::messages::GetStickerSet {
                stickerset: tl::enums::InputStickerSet::ShortName(
                    tl::types::InputStickerSetShortName {
                        short_name: short_name.to_string(),
                    },
                ),
                hash: 0,
            })
            .await?;

        match result {
            tl::enums::messages::StickerSet::Set(set) => {
                let (title, set_short_name) = match &set.set {
                    tl::enums::StickerSet::Set(s) => (s.title.clone(), s.short_name.clone()),
                };

                // Build emoticon lookup: document_id -> emoticon
                let mut emoticon_map = std::collections::HashMap::new();
                for pack in &set.packs {
                    let tl::enums::StickerPack::Pack(p) = pack;
                    for &doc_id in &p.documents {
                        emoticon_map.entry(doc_id).or_insert_with(|| p.emoticon.clone());
                    }
                }

                // Build emoji info from documents (has download data)
                let mut emojis = Vec::new();
                for doc in &set.documents {
                    if let tl::enums::Document::Document(d) = doc {
                        let emoticon = emoticon_map
                            .get(&d.id)
                            .cloned()
                            .unwrap_or_default();

                        emojis.push(EmojiInfo {
                            document_id: d.id,
                            access_hash: d.access_hash,
                            file_reference: d.file_reference.clone(),
                            mime_type: d.mime_type.clone(),
                            emoticon,
                        });
                    }
                }

                Ok(EmojiPackInfo {
                    title,
                    short_name: set_short_name,
                    emojis,
                })
            }
            tl::enums::messages::StickerSet::NotModified => {
                Err(ProviderError::ChatNotFound("Sticker set not modified".into()))
            }
        }
    }

    pub async fn search_custom_emoji(
        &self,
        emoticon: &str,
    ) -> Result<Vec<i64>, ProviderError> {
        let result = self
            .client
            .invoke(&tl::functions::messages::SearchCustomEmoji {
                emoticon: emoticon.to_string(),
                hash: 0,
            })
            .await?;

        match result {
            tl::enums::EmojiList::List(list) => Ok(list.document_id),
            tl::enums::EmojiList::NotModified => Ok(Vec::new()),
        }
    }

    pub async fn fetch_members(
        &self,
        channel: &tl::types::InputChannel,
    ) -> Result<Vec<MemberInfo>, ProviderError> {
        let mut members = Vec::new();
        let mut offset = 0i32;
        let limit = 200;

        loop {
            let result = self
                .client
                .invoke(&tl::functions::channels::GetParticipants {
                    channel: tl::enums::InputChannel::Channel(channel.clone()),
                    filter: tl::enums::ChannelParticipantsFilter::ChannelParticipantsRecent,
                    offset,
                    limit,
                    hash: 0,
                })
                .await?;

            match result {
                tl::enums::channels::ChannelParticipants::Participants(p) => {
                    if p.users.is_empty() {
                        break;
                    }
                    for user in &p.users {
                        if let tl::enums::User::User(u) = user {
                            if !u.bot {
                                members.push(MemberInfo {
                                    user_id: u.id,
                                    username: u.username.clone(),
                                    first_name: u.first_name.clone().unwrap_or_default(),
                                });
                            }
                        }
                    }
                    if (p.users.len() as i32) < limit {
                        break;
                    }
                    offset += p.users.len() as i32;
                }
                tl::enums::channels::ChannelParticipants::NotModified => break,
            }
        }

        Ok(members)
    }

    pub async fn download_emoji(
        &self,
        emoji: &EmojiInfo,
        dir: &Path,
    ) -> Result<std::path::PathBuf, ProviderError> {
        let ext = match emoji.mime_type.as_str() {
            "image/webp" => "webp",
            "application/x-tgsticker" => "tgs",
            "video/webm" => "webm",
            _ => "bin",
        };
        let path = dir.join(format!("{}.{}", emoji.document_id, ext));

        let location = tl::enums::InputFileLocation::InputDocumentFileLocation(
            tl::types::InputDocumentFileLocation {
                id: emoji.document_id,
                access_hash: emoji.access_hash,
                file_reference: emoji.file_reference.clone(),
                thumb_size: String::new(),
            },
        );

        // Download in chunks (max 1MB per request)
        let mut buf = Vec::new();
        let mut offset = 0i64;
        let limit = 1024 * 1024; // 1MB
        loop {
            let result = self
                .client
                .invoke(&tl::functions::upload::GetFile {
                    precise: true,
                    cdn_supported: false,
                    location: location.clone(),
                    offset,
                    limit,
                })
                .await?;

            match result {
                tl::enums::upload::File::File(file) => {
                    if file.bytes.is_empty() {
                        break;
                    }
                    let len = file.bytes.len();
                    buf.extend_from_slice(&file.bytes);
                    if (len as i32) < limit {
                        break;
                    }
                    offset += len as i64;
                }
                tl::enums::upload::File::CdnRedirect(_) => {
                    return Err(ProviderError::Auth("CDN redirect not supported".into()));
                }
            }
        }

        std::fs::write(&path, &buf)
            .map_err(|e| ProviderError::Auth(format!("Failed to write {}: {e}", path.display())))?;

        Ok(path)
    }
}

#[derive(Debug)]
pub struct EmojiPackInfo {
    pub title: String,
    pub short_name: String,
    pub emojis: Vec<EmojiInfo>,
}

#[derive(Debug)]
pub struct EmojiInfo {
    pub document_id: i64,
    pub access_hash: i64,
    pub file_reference: Vec<u8>,
    pub mime_type: String,
    pub emoticon: String,
}

#[derive(Debug, serde::Serialize)]
pub struct MemberInfo {
    pub user_id: i64,
    pub username: Option<String>,
    pub first_name: String,
}

#[derive(Debug)]
pub struct GroupState {
    pub chat_id: i64,
    pub title: String,
    pub description: String,
    pub permissions: Option<PermissionsConfig>,
}

#[derive(Debug)]
pub struct TopicState {
    pub id: i32,
    pub title: String,
    pub icon_emoji_id: Option<i64>,
    pub closed: bool,
}

fn rand_id() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i64
}
