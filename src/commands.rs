use crate::api::{ask_shapesai, APP_ID};
use crate::config::Config;
use crate::errors::YuchiError;
use crate::ui::{display_command_result, display_progress, display_response};
use indicatif::ProgressBar;
use reqwest::blocking::Client;
use serde_json::json;
use uuid::Uuid;
use rpassword::prompt_password;
use std::process::Command;
use colored::Colorize;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use regex::Regex;

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
        let test_response = ask_shapesai("Test", Some(&key), None, "shapesinc/ariwa", user_id, channel_id, None, Some(&pb))?;
        pb.finish_and_clear();

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
            pb.finish_and_clear();
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

        let test_response = ask_shapesai("Test", None, Some(user_auth_token), "shapesinc/ariwa", user_id, channel_id, None, Some(&pb))?;
        pb.finish_and_clear();

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
        ask_shapesai("Test", None, Some(user_auth_token), &model, &user_id, &channel_id, None, Some(&pb))?
    } else if let Some(api_key) = &config.api_key {
        ask_shapesai("Test", Some(api_key), None, &model, &user_id, &channel_id, None, Some(&pb))?
    } else {
        return Err(YuchiError::Config("No API key or user auth token set. Run `yuchi --login` first.".to_string()));
    };
    pb.finish_and_clear();

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

pub fn ask(question: &str, model_override: Option<&str>, image_path: Option<&str>) -> Result<String, YuchiError> {
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
        ask_shapesai(question, None, Some(user_auth_token), &model, &user_id, &channel_id, image_path, Some(&pb))?
    } else if let Some(api_key) = &config.api_key {
        ask_shapesai(question, Some(api_key), None, &model, &user_id, &channel_id, image_path, Some(&pb))?
    } else {
        return Err(YuchiError::Config("No API key or user auth token set. Run `yuchi --login` first.".to_string()));
    };
    pb.finish_and_clear();

    display_response(question, &reply);
    Ok(reply)
}

pub fn run_tool(command: &str, pb: Option<&ProgressBar>) -> Result<(String, bool), YuchiError> {
    let current_dir = std::env::current_dir()
        .map_err(|e| YuchiError::Tool(e.to_string()))?
        .to_string_lossy()
        .into_owned();

    let confirmation = prompt_password(format!("Run `{}` in {}? (y/n): ", command, current_dir))
        .map_err(|e| YuchiError::Input(e.to_string()))?;
    if confirmation.trim().to_lowercase() != "y" {
        let result = "Command execution cancelled by user.".to_string();
        display_command_result(command, &result);
        return Ok((result, false));
    }

    let pb = pb.map(|p| p.clone()).unwrap_or_else(|| display_progress());

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
    let success = output.status.success();

    let result = if success {
        format!("`{}` succeeded:\n{}", command, stdout)
    } else {
        format!("`{}` failed:\n{}", command, stderr)
    };

    display_command_result(command, &result);
    pb.finish_and_clear();

    Ok((result, success))
}

pub fn download_image(response: &str) -> Result<(), YuchiError> {
    // Use regex to find a URL in the response
    let re = Regex::new(r"https://files\.shapes\.inc/[^\s]+")
        .map_err(|e| YuchiError::Api(format!("Failed to compile regex: {}", e)))?;
    let url = re
        .find(response)
        .map(|m| m.as_str())
        .ok_or_else(|| YuchiError::Api("No valid image URL found in response".to_string()))?;

    let client = Client::new();
    let pb = display_progress();
    pb.set_message("Downloading image...");

    let res = client
        .get(url)
        .send()
        .map_err(|e| YuchiError::Api(format!("Failed to download image: {}", e)))?;

    if !res.status().is_success() {
        pb.finish_and_clear();
        return Err(YuchiError::Api(format!("Failed to download image, status: {}", res.status())));
    }

    let bytes = res
        .bytes()
        .map_err(|e| YuchiError::Api(format!("Failed to read image bytes: {}", e)))?;

    // Generate a unique filename using UUID
    let filename = format!("/sdcard/yuchi_image_{}.png", Uuid::new_v4());
    let path = Path::new(&filename);

    let mut file = File::create(path)
        .map_err(|e| YuchiError::Api(format!("Failed to create file '{}': {}", filename, e)))?;

    file.write_all(&bytes)
        .map_err(|e| YuchiError::Api(format!("Failed to write image to '{}': {}", filename, e)))?;

    pb.finish_and_clear();
    println!("{}", format!("Image saved as '{}'", filename).green());

    Ok(())
}
