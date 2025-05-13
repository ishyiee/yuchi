use clap::Parser;
use colored::*;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::process::Command;
use thiserror::Error;
use uuid::Uuid;
use rpassword::prompt_password;
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;

// Hardcoded app_id for user auth token flow 
const APP_ID: &str = "3718bde3-c803-4bfc-b41b-3b5f0aa0ddd8";

#[derive(Error, Debug)]
enum YuchiError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("API error: {0}")]
    Api(String),
    #[error("Tool execution error: {0}")]
    Tool(String),
    #[error("Invalid input: {0}")]
    Input(String),
}

// tool_schemas to run executable shell commands
fn tool_schemas() -> Vec<Value> {
    vec![json!({
        "type": "function",
        "function": {
            "name": "run_shell_command",
            "description": "Run a shell command in the current directory",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to run (e.g., npm install express)"
                    }
                },
                "required": ["command"]
            }
        }
    })]
}

// Execute a shell command
fn run_tool(command: &str) -> Result<String, YuchiError> {
    let current_dir = env::current_dir()
        .map_err(|e| YuchiError::Tool(e.to_string()))?
        .to_string_lossy()
        .into_owned();

    // Prompting User for Safety
    let confirmation = prompt_password(format!("Run `{}` in {}? (y/n): ", command, current_dir))
        .map_err(|e| YuchiError::Input(e.to_string()))?;
    if confirmation.trim().to_lowercase() != "y" {
        return Ok("Command execution cancelled by user.".to_string());
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

    if output.status.success() {
        Ok(format!("`{}` succeeded:\n{}", command, stdout))
    } else {
        Ok(format!("`{}` failed:\n{}", command, stderr))
    }
}

#[derive(Parser)]
#[command(
    name = "yuchi",
    about = "A CLI assistant powered by ShapesAI",
    version = "0.1.0",
    after_help = "To authenticate, run `yuchi --login` and choose:\n\
                  - Option 1: Enter your ShapesAI API key.\n\
                  - Option 2: Use the built-in App ID, visit the authorization URL, and paste the one-time code.\n\
                  Credentials are stored securely in a platform-appropriate config directory."
)]
struct Cli {
    /// Log in by setting an API key or user auth token
    #[arg(long)]
    login: bool,
    /// Set a ShapesAI username to use a custom model (shapesinc/<username>)
    #[arg(long, value_name = "USERNAME")]
    shape: Option<String>,
    /// Clear the stored API key, app ID, auth token, username, user ID, and channel ID
    #[arg(long)]
    logout: bool,
    /// Temporarily override the model for this question
    #[arg(long, value_name = "MODEL")]
    model: Option<String>,
    // prompt to the assistant
    #[arg(value_name = "QUESTION", trailing_var_arg = true, allow_hyphen_values = true)]
    question: Vec<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct Config {
    api_key: Option<String>,
    app_id: Option<String>,
    user_auth_token: Option<String>,
    username: Option<String>,
    user_id: Option<String>,
    channel_id: Option<String>,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{}", format!("Error: {}", e).red());
        std::process::exit(1);
    }
}

fn run() -> Result<(), YuchiError> {
    let cli = Cli::parse();

    if cli.login {
        return login();
    }

    if let Some(username) = cli.shape {
        return set_shape(&username);
    }

    if cli.logout {
        let config = Config::default();
        confy::store("yuchi", None, &config)
            .map_err(|e| YuchiError::Config(e.to_string()))?;
        println!("{}", "API key, app ID, auth token, username, user ID, and channel ID cleared!".green());
        return Ok(());
    }

    if !cli.question.is_empty() {
        let question = cli.question.join(" ");
        return ask(&question, cli.model.as_deref());
    }

    println!("{}", "Error: No command or question provided. Try `yuchi --help`.".magenta());
    Cli::parse_from(["yuchi", "--help"]);
    Ok(())
}

fn login() -> Result<(), YuchiError> {
    let mut config: Config = confy::load("yuchi", None)
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

    // Prompt for authentication method
    let auth_method = prompt_password("Choose authentication method (1: API key, 2: User auth token): ")
        .map_err(|e| YuchiError::Input(e.to_string()))?;
    let auth_method = auth_method.trim();

    if auth_method == "1" {
        // API key flow
        let key = prompt_password("Enter API key: ")
            .map_err(|e| YuchiError::Input(e.to_string()))?;
        if key.trim().is_empty() {
            return Err(YuchiError::Input("API key cannot be empty".to_string()));
        }

        // user_id and channel_id for base apiKey
        if config.user_id.is_none() {
            config.user_id = Some(Uuid::new_v4().to_string());
            println!("{}", "Generated new user ID.".yellow());
        }
        if config.channel_id.is_none() {
            config.channel_id = Some(Uuid::new_v4().to_string());
            println!("{}", "Generated new channel ID.".yellow());
        }
        confy::store("yuchi", None, &config)
            .map_err(|e| YuchiError::Config(e.to_string()))?;

        let user_id = config.user_id.as_ref().unwrap();
        let channel_id = config.channel_id.as_ref().unwrap();
        let test_response = ask_shapesai("Test", Some(&key), None, "shapesinc/ariwa", user_id, channel_id)?;
        if test_response.is_empty() {
            return Err(YuchiError::Api("API key validation failed: No response received".to_string()));
        }

        config.api_key = Some(key);
        config.app_id = None;
        config.user_auth_token = None;
        confy::store("yuchi", None, &config)
            .map_err(|e| YuchiError::Config(e.to_string()))?;
        println!("{}", "API key validated and saved successfully!".green());
    } else if auth_method == "2" {
        // User auth token flow
        let app_id = APP_ID;

        // Save app_id to config
        config.app_id = Some(app_id.to_string());
        confy::store("yuchi", None, &config)
            .map_err(|e| YuchiError::Config(e.to_string()))?;

        // Generate user_id and channel_id if not set
        if config.user_id.is_none() {
            config.user_id = Some(Uuid::new_v4().to_string());
            println!("{}", "Generated new user ID.".yellow());
        }
        if config.channel_id.is_none() {
            config.channel_id = Some(Uuid::new_v4().to_string());
            println!("{}", "Generated new channel ID.".yellow());
        }
        confy::store("yuchi", None, &config)
            .map_err(|e| YuchiError::Config(e.to_string()))?;

        let user_id = config.user_id.as_ref().unwrap();
        let channel_id = config.channel_id.as_ref().unwrap();

        // prompt user to authorize app
        println!("{}", "Click on the link to authorize the application:".yellow());
        println!("{}", format!("https://shapes.inc/authorize?app_id={}", app_id).blue());

        // Prompt for one-time code
        println!("\nAfter logging in to ShapesAI and approving the authorization request,");
        println!("you will be given a one-time code. Copy and paste that code here.");
        let code = prompt_password("Enter the one-time code: ")
            .map_err(|e| YuchiError::Input(e.to_string()))?;
        if code.trim().is_empty() {
            return Err(YuchiError::Input("One-time code cannot be empty".to_string()));
        }

        // Exchange one-time code for user auth token
        let client = Client::new();
        let response = client
            .post("https://api.shapes.inc/auth/nonce")
            .json(&json!({
                "app_id": app_id,
                "code": code
            }))
            .send()
            .map_err(|e| YuchiError::Api(format!("Failed to exchange one-time code: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().unwrap_or_else(|_| "No response body".to_string());
            return Err(YuchiError::Api(format!("Failed to exchange one-time code with status: {}. Response: {}", status, error_body)));
        }

        let response_json: Value = response.json()
            .map_err(|e| YuchiError::Api(format!("Failed to parse auth token response: {}", e)))?;
        let user_auth_token = response_json
            .get("auth_token")
            .and_then(|t| t.as_str())
            .ok_or_else(|| YuchiError::Api("Missing auth_token in response".to_string()))?;

        // Test With Auth Token
        let test_response = ask_shapesai("Test", None, Some(user_auth_token), "shapesinc/ariwa", user_id, channel_id)?;
        if test_response.is_empty() {
            return Err(YuchiError::Api("User auth token validation failed: No response received".to_string()));
        }

        // Store the user auth token
        config.user_auth_token = Some(user_auth_token.to_string());
        config.api_key = None;
        confy::store("yuchi", None, &config)
            .map_err(|e| YuchiError::Config(e.to_string()))?;
        println!("{}", "User auth token validated and saved successfully!".green());
    } else {
        return Err(YuchiError::Input("Invalid authentication method. Choose 1 for API key or 2 for user auth token.".to_string()));
    }

    Ok(())
}

fn set_shape(username: &str) -> Result<(), YuchiError> {
    let config: Config = confy::load("yuchi", None)
        .map_err(|e| YuchiError::Config(e.to_string()))?;
    let user_id = config.user_id
        .ok_or_else(|| YuchiError::Config("No user ID set. Run `yuchi --login` first.".to_string()))?;
    let channel_id = config.channel_id
        .ok_or_else(|| YuchiError::Config("No channel ID set. Run `yuchi --login` first.".to_string()))?;

    let model = format!("shapesinc/{}", username);
    let test_response = if let Some(user_auth_token) = &config.user_auth_token {
        ask_shapesai("Test", None, Some(user_auth_token), &model, &user_id, &channel_id)?
    } else if let Some(api_key) = &config.api_key {
        ask_shapesai("Test", Some(api_key), None, &model, &user_id, &channel_id)?
    } else {
        return Err(YuchiError::Config("No API key or user auth token set. Run `yuchi --login` first.".to_string()));
    };

    if test_response.is_empty() {
        return Err(YuchiError::Api("Username validation failed: No response received.".to_string()));
    }

    let mut config: Config = confy::load("yuchi", None)
        .map_err(|e| YuchiError::Config(e.to_string()))?;
    config.username = Some(username.to_string());
    confy::store("yuchi", None, &config)
        .map_err(|e| YuchiError::Config(e.to_string()))?;
    println!("{}", format!("Username '{}' validated and saved successfully! Using model: {}", username, model).green());
    Ok(())
}

fn ask(question: &str, model_override: Option<&str>) -> Result<(), YuchiError> {
    let config: Config = confy::load("yuchi", None)
        .map_err(|e| YuchiError::Config(e.to_string()))?;
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

    let reply = if let Some(user_auth_token) = &config.user_auth_token {
        ask_shapesai(question, None, Some(user_auth_token), &model, &user_id, &channel_id)?
    } else if let Some(api_key) = &config.api_key {
        ask_shapesai(question, Some(api_key), None, &model, &user_id, &channel_id)?
    } else {
        return Err(YuchiError::Config("No API key or user auth token set. Run `yuchi --login` first.".to_string()));
    };

    println!(
        "\n{} {}\n",
        "yuchi:".bold().bright_magenta(),
        reply.cyan()
    );
    Ok(())
}

fn ask_shapesai(prompt: &str, api_key: Option<&str>, user_auth_token: Option<&str>, model: &str, user_id: &str, channel_id: &str) -> Result<String, YuchiError> {
    let client = Client::new();
    let mut messages = vec![json!({
        "role": "user",
        "content": prompt
    })];

    let mut request_builder = client.post("https://api.shapes.inc/v1/chat/completions");

    if let Some(user_auth_token) = user_auth_token {
        let app_id = confy::load::<Config>("yuchi", None)
            .map_err(|e| YuchiError::Config(e.to_string()))?
            .app_id
            .ok_or_else(|| YuchiError::Config("No app ID set for user auth token.".to_string()))?;
        // Only send X-App-ID and X-User-Auth for user auth token
        request_builder = request_builder
            .header("X-App-ID", app_id)
            .header("X-User-Auth", user_auth_token);
    } else if let Some(api_key) = api_key {
        // Send X-User-ID, X-Channel-ID, and Authorization for API key
        request_builder = request_builder
            .header("X-User-ID", user_id)
            .header("X-Channel-ID", channel_id)
            .header("Authorization", format!("Bearer {}", api_key));
    } else {
        return Err(YuchiError::Api("No API key or user auth token provided.".to_string()));
    }

    request_builder = request_builder.json(&json!({
        "model": model,
        "messages": messages,
        "tools": tool_schemas(),
        "tool_choice": "auto"
    }));

    let res = request_builder
        .send()
        .map_err(|e| YuchiError::Api(format!("Failed to send request to ShapesAI API: {}", e)))?;

    if !res.status().is_success() {
        let status = res.status();
        let error_body = res.text().unwrap_or_else(|_| "No response body".to_string());
        return Err(YuchiError::Api(match status.as_u16() {
            429 => "Blame Shapes, I got rate-limited. Try again later.".to_string(),
            404 => "The resource couldn't be found.".to_string(),
            403 => "I don't have access to the AccessVerse.".to_string(),
            _ => format!("API request failed with status: {}. Response: {}", status, error_body),
        }));
    }

    let json: Value = res.json()
        .map_err(|e| YuchiError::Api(format!("Failed to parse API response: {}", e)))?;

    let tool_calls = json
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("tool_calls"))
        .and_then(|tool_calls| tool_calls.as_array());

    if let Some(tool_calls) = tool_calls {
        messages.push(json!({
            "role": "assistant",
            "tool_calls": tool_calls
        }));

        for tool_call in tool_calls {
            let tool_call_id = tool_call.get("id")
                .and_then(|id| id.as_str())
                .ok_or_else(|| YuchiError::Api("Missing tool call ID".to_string()))?;
            let arguments = tool_call
                .get("function")
                .and_then(|f| f.get("arguments"))
                .ok_or_else(|| YuchiError::Api("Missing tool arguments".to_string()))?;
            let args_str = arguments.as_str()
                .ok_or_else(|| YuchiError::Api("Tool arguments must be a JSON string".to_string()))?;
            let args: serde_json::Map<String, Value> = serde_json::from_str(args_str)
                .map_err(|e| YuchiError::Api(format!("Failed to parse tool arguments: {}", e)))?;
            let command = args
                .get("command")
                .and_then(|c| c.as_str())
                .ok_or_else(|| YuchiError::Api("Missing command parameter".to_string()))?;

            let tool_result = run_tool(command)?;
            messages.push(json!({
                "role": "tool",
                "tool_call_id": tool_call_id,
                "content": tool_result
            }));
        }

        let mut second_request = client.post("https://api.shapes.inc/v1/chat/completions");

        if let Some(user_auth_token) = user_auth_token {
            let app_id = confy::load::<Config>("yuchi", None)
                .map_err(|e| YuchiError::Config(e.to_string()))?
                .app_id
                .ok_or_else(|| YuchiError::Config("No app ID set for user auth token.".to_string()))?;
            // Only send X-App-ID and X-User-Auth for user auth token
            second_request = second_request
                .header("X-App-ID", app_id)
                .header("X-User-Auth", user_auth_token);
        } else if let Some(api_key) = api_key {
            // Send X-User-ID, X-Channel-ID, and Authorization for API key
            second_request = second_request
                .header("X-User-ID", user_id)
                .header("X-Channel-ID", channel_id)
                .header("Authorization", format!("Bearer {}", api_key));
        } else {
            return Err(YuchiError::Api("No API key or user auth token provided.".to_string()));
        }

        second_request = second_request.json(&json!({
            "model": model,
            "messages": messages,
            "tool_choice": "none"
        }));

        let second_res = second_request
            .send()
            .map_err(|e| YuchiError::Api(format!("Failed to send second request to ShapesAI API: {}", e)))?;

        if !second_res.status().is_success() {
            let status = second_res.status();
            let error_body = second_res.text().unwrap_or_else(|_| "No response body".to_string());
            return Err(YuchiError::Api(format!("Second API request failed with status: {}. Response: {}", status, error_body)));
        }

        let second_json: Value = second_res.json()
            .map_err(|e| YuchiError::Api(format!("Failed to parse second API response: {}", e)))?;
        let reply = second_json
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_str())
            .unwrap_or("No response from tool execution.")
            .to_string();

        return Ok(reply);
    }

    let content = json
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .unwrap_or("");

    if content.starts_with("<function>") && content.ends_with("</function>") {
        let command = content
            .strip_prefix("<function>")
            .and_then(|s| s.strip_suffix("</function>"))
            .ok_or_else(|| YuchiError::Api("Invalid function tag format".to_string()))?;

        let tool_result = run_tool(command)?;
        messages.push(json!({
            "role": "tool",
            "tool_call_id": "fallback",
            "content": tool_result
        }));

        let mut second_request = client.post("https://api.shapes.inc/v1/chat/completions");

        if let Some(user_auth_token) = user_auth_token {
            let app_id = confy::load::<Config>("yuchi", None)
                .map_err(|e| YuchiError::Config(e.to_string()))?
                .app_id
                .ok_or_else(|| YuchiError::Config("No app ID set for user auth token.".to_string()))?;
            // Only send X-App-ID and X-User-Auth for user auth token
            second_request = second_request
                .header("X-App-ID", app_id)
                .header("X-User-Auth", user_auth_token);
        } else if let Some(api_key) = api_key {
            // Send X-User-ID, X-Channel-ID, and Authorization for API key
            second_request = second_request
                .header("X-User-ID", user_id)
                .header("X-Channel-ID", channel_id)
                .header("Authorization", format!("Bearer {}", api_key));
        } else {
            return Err(YuchiError::Api("No API key or user auth token provided.".to_string()));
        }

        second_request = second_request.json(&json!({
            "model": model,
            "messages": messages,
            "tool_choice": "none"
        }));

        let second_res = second_request
            .send()
            .map_err(|e| YuchiError::Api(format!("Failed to send second request to ShapesAI API: {}", e)))?;

        if !second_res.status().is_success() {
            let status = second_res.status();
            let error_body = second_res.text().unwrap_or_else(|_| "No response body".to_string());
            return Err(YuchiError::Api(format!("Second API request failed with status: {}. Response: {}", status, error_body)));
        }

        let second_json: Value = second_res.json()
            .map_err(|e| YuchiError::Api(format!("Failed to parse second API response: {}", e)))?;
        let reply = second_json
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_str())
            .unwrap_or("No response from tool execution.")
            .to_string();

        return Ok(reply);
    }

    Ok(content.to_string())
}