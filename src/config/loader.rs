use std::path::Path;

use crate::error::ConfigError;

use super::schema::Config;

pub fn load_config(path: &Path) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    validate(&config)?;
    Ok(config)
}

fn validate(config: &Config) -> Result<(), ConfigError> {
    if config.provider.api_hash.is_empty() {
        return Err(ConfigError::Validation(
            "provider.api_hash cannot be empty".into(),
        ));
    }

    for (name, group) in &config.group {
        if group.chat.is_empty() {
            return Err(ConfigError::Validation(format!(
                "group.{name}.chat cannot be empty"
            )));
        }

        let mut topic_titles = std::collections::HashSet::new();
        for topic in &group.topic {
            if !topic_titles.insert(&topic.title) {
                return Err(ConfigError::Validation(format!(
                    "group.{name}: duplicate topic title {:?}",
                    topic.title
                )));
            }
        }
    }

    Ok(())
}
