# tgctl

Declarative Telegram group management. Define your group's topics, permissions, and description in TOML — then plan and apply changes like Terraform.

Uses Telegram Client API (MTProto) via [grammers](https://codeberg.org/Lonami/grammers), not Bot API — can read forum topics, full permissions, and more.

## Setup

1. Get `api_id` and `api_hash` from https://my.telegram.org
2. Copy `telegram.example.toml` to `telegram.toml` and fill in credentials
3. First run will prompt for phone number + code (+ 2FA if enabled). Session persists in `tgctl.session`.

## Usage

```bash
# Validate config syntax
tgctl validate

# Import existing group state
tgctl import --chat -1003744795236 --name my-community

# Show what would change
tgctl plan

# Apply changes
tgctl apply
tgctl apply --auto-approve

# List custom emoji from a pack (for icon_emoji_id)
tgctl emoji list --pack <pack_short_name>

# Search custom emoji by emoticon
tgctl emoji search --query "📢"
```

## Config (`telegram.toml`)

```toml
[provider]
api_id = 12345
api_hash = "your_api_hash"

[group."my-community"]
chat = "-1003744795236"
title = "My Community"
description = "A community for discussing things."

[group."my-community".permissions]
send_messages = true
send_media = true
send_stickers = true
send_gifs = true
send_polls = true
embed_links = true
invite_users = true
pin_messages = false
change_info = false

[[group."my-community".topic]]
title = "General"
closed = false

[[group."my-community".topic]]
title = "Announcements"
icon_emoji_id = 5368324170671202286
closed = false
```

## Managed resources

- Group title and description
- Default member permissions
- Forum topics (create/edit/close, matched by title)
- Custom emoji icons on topics (`icon_emoji_id`)

Topics not in config are flagged for deletion in `plan` but not removed unless explicitly deleted.

## Building

```bash
cargo build --release
```
