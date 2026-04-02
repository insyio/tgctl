mod cli;
mod config;
mod diff;
mod error;
mod provider;
mod resources;
mod state;

use anyhow::{Context, Result};
use base64::Engine;
use clap::Parser;
use colored::Colorize;
use dialoguer::Confirm;
use grammers_tl_types as tl;

use cli::{Cli, Command, EmojiAction};
use config::loader::load_config;
use diff::actions::Action;
use diff::plan::display_plan;
use provider::telegram::TelegramProvider;
use resources::forum_topic::topic_to_state;
use resources::group::group_to_state;
use state::statefile::{load_state, save_state};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Command::Validate => cmd_validate(&cli),
        Command::Plan => cmd_plan(&cli).await,
        Command::Apply { auto_approve } => cmd_apply(&cli, auto_approve).await,
        Command::Import { ref chat, ref name } => cmd_import(&cli, chat, name).await,
        Command::Emoji { ref action } => match action {
            EmojiAction::List { ref pack } => cmd_emoji_list(&cli, pack).await,
            EmojiAction::Search { ref query } => cmd_emoji_search(&cli, query).await,
        },
    }
}

fn cmd_validate(cli: &Cli) -> Result<()> {
    let config = load_config(&cli.config).context("Failed to load config")?;
    println!(
        "{} Config is valid ({} group(s) defined)",
        "✓".green(),
        config.group.len()
    );
    Ok(())
}

async fn cmd_plan(cli: &Cli) -> Result<()> {
    let config = load_config(&cli.config).context("Failed to load config")?;
    let provider = TelegramProvider::connect(&config.provider).await?;

    let all_actions = compute_plan(&config, &provider).await?;
    display_plan(&all_actions);

    Ok(())
}

async fn cmd_apply(cli: &Cli, auto_approve: bool) -> Result<()> {
    let config = load_config(&cli.config).context("Failed to load config")?;
    let provider = TelegramProvider::connect(&config.provider).await?;

    let all_actions = compute_plan(&config, &provider).await?;
    display_plan(&all_actions);

    let has_changes = all_actions
        .iter()
        .any(|a| !matches!(a, Action::NoOp(_)));

    if !has_changes {
        return Ok(());
    }

    if !auto_approve {
        let confirmed = Confirm::new()
            .with_prompt("Do you want to apply these changes?")
            .default(false)
            .interact()
            .context("Failed to read confirmation")?;

        if !confirmed {
            println!("Apply cancelled.");
            return Ok(());
        }
    }

    // Apply changes
    let mut state = load_state(&cli.state).context("Failed to load state")?;

    for (group_name, group_config) in &config.group {
        let channel = provider.resolve_chat(&group_config.chat).await?;
        let actions = compute_group_plan(group_name, group_config, &provider, &channel).await?;

        for action in &actions {
            match action {
                Action::NoOp(_) => {}
                _ => {
                    let key = match action {
                        Action::Create(p) | Action::Update(p) | Action::Delete(p) => {
                            &p.resource_key
                        }
                        Action::NoOp(_) => unreachable!(),
                    };
                    println!("  Applying: {}", key.bold());
                    provider.apply_action(action, &channel).await?;
                    println!("  {}", "Done".green());
                }
            }
        }

        // Refresh and save state after applying
        let live_group = provider.fetch_group(&channel).await?;
        let live_topics = provider.fetch_topics(&channel).await?;

        let group_key = format!("group.{group_name}");
        state
            .resources
            .insert(group_key, group_to_state(group_name, &live_group));

        // Remove old topic entries for this group
        let topic_prefix = format!("forum_topic.{group_name}.");
        state
            .resources
            .retain(|k, _| !k.starts_with(&topic_prefix));

        for topic in &live_topics {
            let topic_key = format!("forum_topic.{group_name}.{}", topic.title);
            state
                .resources
                .insert(topic_key, topic_to_state(group_name, topic));
        }
    }

    state.serial += 1;
    save_state(&cli.state, &state).context("Failed to save state")?;

    println!(
        "\n{}",
        "Apply complete! State saved.".green().bold()
    );

    Ok(())
}

async fn cmd_import(cli: &Cli, chat: &str, name: &str) -> Result<()> {
    let config = load_config(&cli.config).context("Failed to load config")?;
    let provider = TelegramProvider::connect(&config.provider).await?;

    println!("Importing {}...", chat);

    let channel = provider.resolve_chat(chat).await?;
    let live_group = provider.fetch_group(&channel).await?;
    let live_topics = provider.fetch_topics(&channel).await?;

    let mut state = load_state(&cli.state).context("Failed to load state")?;

    let group_key = format!("group.{name}");
    state
        .resources
        .insert(group_key, group_to_state(name, &live_group));

    for topic in &live_topics {
        let topic_key = format!("forum_topic.{name}.{}", topic.title);
        state
            .resources
            .insert(topic_key, topic_to_state(name, topic));
    }

    state.serial += 1;
    save_state(&cli.state, &state).context("Failed to save state")?;

    println!(
        "{} Imported group {:?} with {} topic(s)",
        "✓".green(),
        live_group.title,
        live_topics.len()
    );
    println!("State saved to {:?}", cli.state);

    Ok(())
}

async fn compute_plan(
    config: &config::schema::Config,
    provider: &TelegramProvider,
) -> Result<Vec<Action>> {
    let mut all_actions = Vec::new();

    for (group_name, group_config) in &config.group {
        let channel = provider.resolve_chat(&group_config.chat).await?;
        let actions =
            compute_group_plan(group_name, group_config, provider, &channel).await?;
        all_actions.extend(actions);
    }

    Ok(all_actions)
}

async fn compute_group_plan(
    group_name: &str,
    group_config: &config::schema::GroupConfig,
    provider: &TelegramProvider,
    channel: &tl::types::InputChannel,
) -> Result<Vec<Action>> {
    let live_group = provider
        .fetch_group(channel)
        .await
        .context(format!("Failed to fetch group {group_name}"))?;

    let live_topics = provider
        .fetch_topics(channel)
        .await
        .context(format!("Failed to fetch topics for {group_name}"))?;

    Ok(diff::diff_group(
        group_name,
        group_config,
        &live_group,
        &live_topics,
    ))
}

async fn cmd_emoji_list(cli: &Cli, pack: &str) -> Result<()> {
    let config = load_config(&cli.config).context("Failed to load config")?;
    let provider = TelegramProvider::connect(&config.provider).await?;

    let info = provider
        .fetch_emoji_pack(pack)
        .await
        .context(format!("Failed to fetch emoji pack {pack:?}"))?;

    // Download emoji files to .emoji-cache/<pack>/
    let cache_dir = std::path::PathBuf::from(format!(".emoji-cache/{}", info.short_name));
    std::fs::create_dir_all(&cache_dir).context("Failed to create cache dir")?;

    println!(
        "Pack: {:?} ({})\n",
        info.title, info.short_name
    );

    let mut html_rows = String::new();

    for (i, emoji) in info.emojis.iter().enumerate() {
        let path = provider
            .download_emoji(emoji, &cache_dir)
            .await
            .context(format!("Failed to download emoji {}", emoji.document_id))?;

        // Try to show inline in terminal via Kitty graphics protocol (not in tmux)
        let in_tmux = std::env::var("TMUX").is_ok();
        let inline_shown = if !in_tmux {
            if let Some(png_data) = emoji_to_png(&path) {
                kitty_display_inline(&png_data).is_ok()
            } else {
                false
            }
        } else {
            false
        };

        let kind = if emoji.mime_type.contains("webm") || emoji.mime_type.contains("tgsticker") {
            "[anim]"
        } else {
            ""
        };

        println!(
            "{}  {:<4} {:<22} {:<6} {}",
            if inline_shown { "" } else { "  " },
            i + 1,
            emoji.document_id,
            emoji.emoticon,
            kind,
        );

        // Build HTML row — PNG for static, video for animated
        let preview = if let Some(png_data) = emoji_to_png(&path) {
            let b64 = base64::prelude::BASE64_STANDARD.encode(&png_data);
            format!("<img src='data:image/png;base64,{}' width='48' height='48'>", b64)
        } else {
            let file_bytes = std::fs::read(&path).unwrap_or_default();
            let b64 = base64::prelude::BASE64_STANDARD.encode(&file_bytes);
            format!(
                "<video src='data:{};base64,{}' width='48' height='48' autoplay loop muted></video>",
                emoji.mime_type, b64
            )
        };
        html_rows.push_str(&format!(
            "<tr><td>{}</td><td>{}</td>\
             <td><code>{}</code></td><td>{}</td></tr>\n",
            i + 1,
            preview,
            emoji.document_id,
            emoji.emoticon,
        ));
    }

    // Generate HTML preview
    let html_path = cache_dir.join("preview.html");
    let html = format!(
        "<!DOCTYPE html><html><head><meta charset='utf-8'>\
         <title>{} — {}</title>\
         <style>body{{font-family:system-ui;background:#1e1e1e;color:#eee;padding:20px}}\
         table{{border-collapse:collapse}}td{{padding:8px 16px;border-bottom:1px solid #333}}\
         code{{background:#333;padding:2px 6px;border-radius:3px;user-select:all}}\
         img{{border-radius:4px;background:#333}}</style></head>\
         <body><h2>{} <small>({})</small></h2>\
         <table><tr><th>#</th><th>preview</th><th>icon_emoji_id</th><th>alias</th></tr>\
         {}</table></body></html>",
        info.title, info.short_name, info.title, info.short_name, html_rows
    );
    std::fs::write(&html_path, &html).context("Failed to write HTML preview")?;

    println!(
        "\n{} emoji. Preview: {}",
        info.emojis.len(),
        std::fs::canonicalize(&html_path)?.display()
    );

    Ok(())
}

async fn cmd_emoji_search(cli: &Cli, query: &str) -> Result<()> {
    let config = load_config(&cli.config).context("Failed to load config")?;
    let provider = TelegramProvider::connect(&config.provider).await?;

    let ids = provider
        .search_custom_emoji(query)
        .await
        .context(format!("Failed to search custom emoji for {query:?}"))?;

    if ids.is_empty() {
        println!("No custom emoji found for {:?}", query);
        return Ok(());
    }

    println!("Custom emoji for {:?}:\n", query);
    println!("{:<4} {}", "#", "icon_emoji_id");
    println!("{}", "-".repeat(30));

    for (i, id) in ids.iter().enumerate() {
        println!("{:<4} {}", i + 1, id);
    }

    println!("\n{} results", ids.len());

    Ok(())
}

/// Convert a WebP/PNG emoji file to PNG bytes
fn emoji_to_png(path: &std::path::Path) -> Option<Vec<u8>> {
    let img = image::open(path).ok()?;
    let img = img.resize(64, 64, image::imageops::FilterType::Lanczos3);
    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    img.write_to(&mut cursor, image::ImageFormat::Png).ok()?;
    Some(buf)
}

/// Display PNG data inline using Kitty graphics protocol
fn kitty_display_inline(png_data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let b64 = base64::prelude::BASE64_STANDARD.encode(png_data);
    let mut stdout = std::io::stdout().lock();

    // Kitty protocol: chunks of 4096 base64 chars
    // First chunk: f=100 (PNG), a=T (transmit+display), s/v = pixel size
    let chunk_size = 4096;
    let total = b64.len();

    if total <= chunk_size {
        write!(stdout, "\x1b_Gf=100,a=T;{}\x1b\\", b64)?;
    } else {
        let mut offset = 0;
        while offset < total {
            let end = (offset + chunk_size).min(total);
            let chunk = &b64[offset..end];
            let more = if end < total { 1 } else { 0 };

            if offset == 0 {
                write!(stdout, "\x1b_Gf=100,a=T,m={};{}\x1b\\", more, chunk)?;
            } else {
                write!(stdout, "\x1b_Gm={};{}\x1b\\", more, chunk)?;
            }
            offset = end;
        }
    }

    stdout.flush()?;
    Ok(())
}
