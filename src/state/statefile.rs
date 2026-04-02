use std::path::Path;

use crate::error::StateError;

use super::types::StateFile;

pub fn load_state(path: &Path) -> Result<StateFile, StateError> {
    if !path.exists() {
        return Ok(StateFile::new());
    }
    let content = std::fs::read_to_string(path)?;
    let state: StateFile = serde_json::from_str(&content)?;
    Ok(state)
}

pub fn save_state(path: &Path, state: &StateFile) -> Result<(), StateError> {
    let content = serde_json::to_string_pretty(state)?;
    std::fs::write(path, content)?;
    Ok(())
}
