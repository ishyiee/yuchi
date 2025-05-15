use crate::errors::YuchiError;
use serde::{Deserialize, Serialize};
use confy::ConfyError;

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    pub api_key: Option<String>,
    pub app_id: Option<String>,
    pub user_auth_token: Option<String>,
    pub username: Option<String>,
    pub user_id: Option<String>,
    pub channel_id: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self, YuchiError> {
        confy::load("yuchi", "config")
            .map_err(|e| YuchiError::Config(format!("Failed to load config: {}", e)))
    }

    pub fn save(&self) -> Result<(), YuchiError> {
        confy::store("yuchi", "config", self)
            .map_err(|e| YuchiError::Config(format!("Failed to save config: {}", e)))
    }
}