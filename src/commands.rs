use crate::api::{ask_shapesai, APP_ID};
use crate::config::Config;
use crate::errors::YuchiError;
use crate::ui::{display_command_result, display_progress, display_response};
use reqwest::blocking::Client;
use serde_json::json;
use uuid::Uuid;
use rpassword::prompt_password;
use std::process::Command;
use colored::Colorize;

pub fn login() -> Result<(), YuchiError> {
    let mut config = Config::load()?;
    let auth_method = prompt_password("Choose authentication method (1: API key, 2: User auth token): ")
        .map_err(|e| YuchiError::Input(e.to_string()))?
        .trim()
        .to_string();

    if auth_method == "1" {
        let key = prompt_password("Enter API key: ")
            .map_err(|e| YuchiError::Input(e.to_string()))?;
        if key.trim().is_empty() {
            return Err(YuchiError::Input("API key cannot be empty".to_string()));
        }

        if config.user_id.is_none() {
            config.user_id = Some(Uuid::new_v4().to_string());
            println!("{}", "Generated new user ID.".yellow());
        }
        if config.channel_id.is_none() {
            config.channel_id = Some(Uuid::new_v4().to_string());
            println!("{}", "Generated new channel ID.".yellow());
        }
        config.save()?;

        let user_id = config.user_id.as_ref().unwrap();
        let channel_id = config.channel_id.as_ref().unwrap();
        let pb = display_progress();
        let test_response = ask_shapesai("Test", Some(&key), None, "shapesinc/ariwa", user_id, channel_id, None)?;
        pb.finish_and_clear(); // Updated: Clear spinner silently

        if test_response.is_empty() {
            return Err(YuchiError::Api("API key validation failed: No response received".to_string()));
        }

        config.api_key = Some(key);
        config.app_id = None;
        config.user_auth_token = None;
        config.save()?;
        println!("{}", "API key validated and saved successfully!".green());
    } else if auth_method == "2" {
        config.app_id = Some(APP_ID.to_string());
        config.save()?;

        if config.user_id.is_none() {
            config.user_id = Some(Uuid::new_v4().to_string());
            println!("{}", "Generated new user ID.".yellow());
        }
        if config.channel_id.is_none() {
            config.channel_id = Some(Uuid::new_v4().to_string());
            println!("{}", "Generated new channel ID.".yellow());
        }
        config.save()?;

        let user_id = config.user_id.as_ref().unwrap();
        let channel_id = config.channel_id.as_ref().unwrap();

        println!("{}", "Click on the link to authorize the application:".yellow());
        println!("{}", format!("https://shapes.inc/authorize?app_id={}", APP_ID).as_str().blue());
        println!("\nAfter logging in to ShapesAI and approving the authorization request,");
        println!("you will be given a one-time code. Copy and paste that code here.");

        let code = prompt_password("Enter the one-time code: ")
            .map_err(|e| YuchiError::Input(e.to_string()))?;
        if code.trim().is_empty() {
            return Err(YuchiError::Input("One-time code cannot be empty".to_string()));
        }

        let pb = display_progress();
        let client = Client::new();
        let response = client
            .post("https://api.shapes.inc/auth/nonce")
            .json(&json!({
                "app_id": APP_ID,
                "code": code
            }))
            .send()
            .map_err(|e| YuchiError::Api(format!("Failed to exchange one-time code: {}", e)))?;

        if !response.status().is_success() {
            pb.finish_and_clear(); // Updated: Clear spinner silently
            let status = response.status();
            let error_body = response.text().unwrap_or_else(|_| "No response body".to_string());
            return Err(YuchiError::Api(format!("Failed to exchange one-time code with status: {}. Response: {}", status, error_body)));
        }

        let response_json = response.json::<serde_json::Value>()
            .map_err(|e| YuchiError::Api(format!("Failed to parse auth token response: {}", e)))?;
        let user_auth_token = response_json
            .get("auth_token")
            .and_then(|t| t.as_str())
            .ok_or_else(|| YuchiError::Api("Missing auth_token in response".to_string()))?;

        let test_response = ask_shapesai("Test", None, Some(user_auth_token), "shapesinc/ariwa", user_id, channel_id, None)?;
        pb.finish_and_clear(); // Updated: Clear spinner silently

        if test_response.is_empty() {
            return Err(YuchiError::Api("User auth token validation failed: No response received".to_string()));
        }

        config.user_auth_token = Some(user_auth_token.to_string());
        config.api_key = None;
        config.save()?;
        println!("{}", "User auth token validated and saved successfully!".green());
    } else {
        return Err(YuchiError::Input("Invalid authentication method. Choose 1 for API key or 2 for user auth token.".to_string()));
    }

    Ok(())
}

pub fn set_shape(username: &str) -> Result<(), YuchiError> {
    let config = Config::load()?;
    let user_id = config.user_id
        .ok_or_else(|| YuchiError::Config("No user ID set. Run `yuchi --login` first.".to_string()))?;
    let channel_id = config.channel_id
        .ok_or_else(|| YuchiError::Config("No channel ID set. Run `yuchi --login` first.".to_string()))?;

    let model = format!("shapesinc/{}", username);
    let pb = display_progress();
    let test_response = if let Some(user_auth_token) = &config.user_auth_token {
        ask_shapesai("Test", None, Some(user_auth_token), &model, &user_id, &channel_id, None)?
    } else if let Some(api_key) = &config.api_key {
        ask_shapesai("Test", Some(api_key), None, &model, &user_id, &channel_id, None)?
    } else {
        return Err(YuchiError::Config("No API key or user auth token set. Run `yuchi --login` first.".to_string()));
    };
    pb.finish_and_clear(); // Updated: Clear spinner silently

    if test_response.is_empty() {
        return Err(YuchiError::Api("Username validation failed: No response received.".to_string()));
    }

    let mut config = Config::load()?;
    config.username = Some(username.to_string());
    config.save()?;
    println!("{}", format!("Username '{}' validated and saved successfully! Using model: {}", username, model).as_str().green());
    Ok(())
}

pub fn logout() -> Result<(), YuchiError> {
    let config = Config::default();
    config.save()?;
    println!("{}", "API key, app ID, auth token, username, user ID, and channel ID cleared!".green());
    Ok(())
}

pub fn ask(question: &str, model_override: Option<&str>, image_path: Option<&str>) -> Result<(), YuchiError> {
    let config = Config::load()?;
    let user_id = config.user_id
        .ok_or_else(|| YuchiError::Config("No user ID set. Run `yuchi --login` first.".to_string()))?;
    let channel_id = config.channel_id
        .ok_or_else(|| YuchiError::Config("No channel ID set. Run `yuchi --login` first.".to_string()))?;

    let default_model = config
        .username
        .as_ref()
        .map(|u| format!("shapesinc/{}", u))
        .unwrap_or_else(|| "shapesinc/ariwa".to_string());
    let model = model_override.unwrap_or(&default_model);

    let pb = display_progress();
    let reply = if let Some(user_auth_token) = &config.user_auth_token {
        ask_shapesai(question, None, Some(user_auth_token), &model, &user_id, &channel_id, image_path)?
    } else if let Some(api_key) = &config.api_key {
        ask_shapesai(question, Some(api_key), None, &model, &user_id, &channel_id, image_path)?
    } else {
        return Err(YuchiError::Config("No API key or user auth token set. Run `yuchi --login` first.".to_string()));
    };
    pb.finish_and_clear(); // Updated: Clear spinner silently

    display_response(question, &reply);
    Ok(())
}

pub fn run_tool(command: &str) -> Result<(), YuchiError> {
    let current_dir = std::env::current_dir()
        .map_err(|e| YuchiError::Tool(e.to_string()))?
        .to_string_lossy()
        .into_owned();

    // Prompting User for Safety
    let confirmation = prompt_password(format!("Run `{}` in {}? (y/n): ", command, current_dir))
        .map_err(|e| YuchiError::Input(e.to_string()))?;
    if confirmation.trim().to_lowercase() != "y" {
        display_command_result(command, "Command execution cancelled by user.");
        return Ok(());
    }

    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return Err(YuchiError::Tool("Empty command".to_string()));
    }
    let (program, args) = (parts[0], &parts[1..]);

    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| YuchiError::Tool(format!("Failed to execute `{}`: {}", command, e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    let result = if output.status.success() {
        format!("`{}` succeeded:\n{}", command, stdout)
    } else {
        format!("`{}` failed:\n{}", command, stderr)
    };

    display_command_result(command, &result);
    Ok(())
}