use crate::errors::YuchiError;
use serde::{Deserialize, Serialize};
use confy;
use std::fs;
use std::os::unix::fs::PermissionsExt;

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
        let config: Config = confy::load("yuchi", None)
            .map_err(|e| YuchiError::Config(e.to_string()))?;

        // Set permissions on config file (Unix-only)
        #[cfg(unix)]
        {
            let config_path = confy::get_configuration_file_path("yuchi", None)
                .map_err(|e| YuchiError::Config(e.to_string()))?;
            if config_path.exists() {
                fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600))
                    .map_err(|e| YuchiError::Config(format!("Failed to set config permissions: {}", e)))?;
            }
        }
        #[cfg(not(unix))]
        println!("{}", "Note: Config file permissions not modified on non-Unix systems.".yellow());

        Ok(config)
    }

    pub fn save(&self) -> Result<(), YuchiError> {
        confy::store("yuchi", None, self)
            .map_err(|e| YuchiError::Config(e.to_string()))?;
        Ok(())
    }
}