use crate::config::Config;
use crate::errors::YuchiError;
use crate::ui::display_progress;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::fs;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use crate::commands::run_tool;

// Hardcoded app_id for user auth token flow
pub const APP_ID: &str = "3718bde3-c803-4bfc-b41b-3b5f0aa0ddd8";

// Define tool schemas for ShapesAI API
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

pub fn ask_shapesai(
    prompt: &str,
    api_key: Option<&str>,
    user_auth_token: Option<&str>,
    model: &str,
    user_id: &str,
    channel_id: &str,
    image_path: Option<&str>,
    pb: Option<&indicatif::ProgressBar>,
) -> Result<String, YuchiError> {
    let client = Client::new();
    let mut messages = vec![];

    // Adjust prompt for text extraction if "text" is in the prompt
    let adjusted_prompt = if image_path.is_some() && prompt.to_lowercase().contains("text") {
        format!("Extract the text from this image: {}", prompt)
    } else {
        prompt.to_string()
    };

    if let Some(image_path) = image_path {
        let path = std::path::Path::new(image_path);
        if !path.exists() || !path.is_file() {
            return Err(YuchiError::Image(format!(
                "Image file '{}' does not exist or is not a file",
                image_path
            )));
        }

        let image_data = fs::read(path).map_err(|e| {
            YuchiError::Image(format!("Failed to read image file '{}': {}", image_path, e))
        })?;
        let base64_image = BASE64.encode(&image_data);

        // Guess MIME type based on extension (PNG or JPEG)
        let mime_type = match path.extension().and_then(|ext| ext.to_str()) {
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            _ => {
                return Err(YuchiError::Image(format!(
                    "Unsupported image format for '{}'. Use PNG or JPEG.",
                    image_path
                )))
            }
        };

        let image_url = format!("data:{};base64,{}", mime_type, base64_image);

        messages.push(json!({
            "role": "user",
            "content": [
                { "type": "text", "text": adjusted_prompt },
                { "type": "image_url", "image_url": { "url": image_url } }
            ]
        }));
    } else {
        messages.push(json!({
            "role": "user",
            "content": adjusted_prompt
        }));
    }

    let mut request_builder = client.post("https://api.shapes.inc/v1/chat/completions");

    if let Some(user_auth_token) = user_auth_token {
        let app_id = Config::load()?
            .app_id
            .ok_or_else(|| YuchiError::Config("No app ID set for user auth token.".to_string()))?;
        request_builder = request_builder
            .header("X-App-ID", app_id)
            .header("X-User-Auth", user_auth_token);
    } else if let Some(api_key) = api_key {
        request_builder = request_builder
            .header("X-User-ID", user_id)
            .header("X-Channel-ID", channel_id)
            .header("Authorization", format!("Bearer {}", api_key));
    } else {
        return Err(YuchiError::Api(
            "No API key or user auth token provided.".to_string(),
        ));
    }

    request_builder = request_builder.json(&json!({
        "model": model,
        "messages": messages,
        "tools": tool_schemas(),
        "tool_choice": "auto"
    }));

    let pb = pb.cloned().unwrap_or_else(|| display_progress());
    pb.set_message("Querying ShapesAI...");

    let res = request_builder.send().map_err(|e| {
        YuchiError::Api(format!("Failed to send request to ShapesAI API: {}", e))
    })?;

    if !res.status().is_success() {
        let status = res.status();
        let error_body = res.text().unwrap_or_else(|_| "No response body".to_string());
        pb.finish_and_clear();
        return Err(YuchiError::Api(match status.as_u16() {
            429 => "Blame Shapes, I got rate-limited. Try again later.".to_string(),
            404 => "The resource couldn't be found.".to_string(),
            403 => "I don't have access to the AccessVerse.".to_string(),
            _ => format!("API request failed with status: {}. Response: {}", status, error_body),
        }));
    }

    let json: Value = res
        .json()
        .map_err(|e| YuchiError::Api(format!("Failed to parse API response: {}", e)))?;

    let tool_calls = json
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("tool_calls"))
        .and_then(|tool_calls| tool_calls.as_array());

    if let Some(tool_calls) = tool_calls {
        pb.finish_and_clear(); // Clear progress bar before tool execution
        messages.push(json!({
            "role": "assistant",
            "tool_calls": tool_calls
        }));

        for tool_call in tool_calls {
            let tool_call_id = tool_call
                .get("id")
                .and_then(|id| id.as_str())
                .ok_or_else(|| YuchiError::Api("Missing tool call ID".to_string()))?;
            let arguments = tool_call
                .get("function")
                .and_then(|f| f.get("arguments"))
                .ok_or_else(|| YuchiError::Api("Missing tool arguments".to_string()))?;
            let args_str = arguments
                .as_str()
                .ok_or_else(|| YuchiError::Api("Tool arguments must be a JSON string".to_string()))?;
            let args: serde_json::Map<String, Value> = serde_json::from_str(args_str).map_err(|e| {
                YuchiError::Api(format!("Failed to parse tool arguments: {}", e))
            })?;
            let command = args
                .get("command")
                .and_then(|c| c.as_str())
                .ok_or_else(|| YuchiError::Api("Missing command parameter".to_string()))?;

            let tool_result = run_tool(command, Some(&pb))?;
            messages.push(json!({
                "role": "tool",
                "tool_call_id": tool_call_id,
                "content": tool_result
            }));
        }

        let mut second_request = client.post("https://api.shapes.inc/v1/chat/completions");

        if let Some(user_auth_token) = user_auth_token {
            let app_id = Config::load()?
                .app_id
                .ok_or_else(|| YuchiError::Config("No app ID set for user auth token.".to_string()))?;
            second_request = second_request
                .header("X-App-ID", app_id)
                .header("X-User-Auth", user_auth_token);
        } else if let Some(api_key) = api_key {
            second_request = second_request
                .header("X-User-ID", user_id)
                .header("X-Channel-ID", channel_id)
                .header("Authorization", format!("Bearer {}", api_key));
        }

        second_request = second_request.json(&json!({
            "model": model,
            "messages": messages,
            "tool_choice": "none"
        }));

        pb.set_message("Querying ShapesAI..."); // Restart progress bar
        let second_res = second_request.send().map_err(|e| {
            YuchiError::Api(format!("Failed to send second request to ShapesAI API: {}", e))
        })?;

        pb.finish_and_clear();

        if !second_res.status().is_success() {
            let status = second_res.status();
            let error_body = second_res
                .text()
                .unwrap_or_else(|_| "No response body".to_string());
            return Err(YuchiError::Api(format!(
                "Second API request failed with status: {}. Response: {}",
                status, error_body
            )));
        }

        let second_json: Value = second_res.json().map_err(|e| {
            YuchiError::Api(format!("Failed to parse second API response: {}", e))
        })?;
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

    // Fallback for <function> tag format
    let content = json
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .unwrap_or("");

    if content.starts_with("<function>") && content.ends_with("</function>") {
        pb.finish_and_clear(); // Clear progress bar before tool execution
        let command = content
            .strip_prefix("<function>")
            .and_then(|s| s.strip_suffix("</function>"))
            .ok_or_else(|| YuchiError::Api("Invalid function tag format".to_string()))?;

        let args: Value = serde_json::from_str(command)
            .map_err(|e| YuchiError::Api(format!("Failed to parse function arguments: {}", e)))?;
        let command = args
            .get("command")
            .and_then(|c| c.as_str())
            .ok_or_else(|| YuchiError::Api("Missing command parameter".to_string()))?;

        let tool_result = run_tool(command, Some(&pb))?;
        messages.push(json!({
            "role": "tool",
            "tool_call_id": "fallback",
            "content": tool_result
        }));

        let mut second_request = client.post("https://api.shapes.inc/v1/chat/completions");

        if let Some(user_auth_token) = user_auth_token {
            let app_id = Config::load()?
                .app_id
                .ok_or_else(|| YuchiError::Config("No app ID set for user auth token.".to_string()))?;
            second_request = second_request
                .header("X-App-ID", app_id)
                .header("X-User-Auth", user_auth_token);
        } else if let Some(api_key) = api_key {
            second_request = second_request
                .header("X-User-ID", user_id)
                .header("X-Channel-ID", channel_id)
                .header("Authorization", format!("Bearer {}", api_key));
        }

        second_request = second_request.json(&json!({
            "model": model,
            "messages": messages,
            "tool_choice": "none"
        }));

        pb.set_message("Querying ShapesAI..."); // Restart progress bar
        let second_res = second_request.send().map_err(|e| {
            YuchiError::Api(format!("Failed to send second request to ShapesAI API: {}", e))
        })?;

        pb.finish_and_clear();

        if !second_res.status().is_success() {
            let status = second_res.status();
            let error_body = second_res
                .text()
                .unwrap_or_else(|_| "No response body".to_string());
            return Err(YuchiError::Api(format!(
                "Second API request failed with status: {}. Response: {}",
                status, error_body
            )));
        }

        let second_json: Value = second_res.json().map_err(|e| {
            YuchiError::Api(format!("Failed to parse second API response: {}", e))
        })?;
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

    pb.finish_and_clear();
    Ok(content.to_string())
}
