mod api;
mod commands;
mod config;
mod errors;
mod ui;

use clap::Parser;
use crate::errors::YuchiError;
use crate::ui::{display_error, display_help};

#[derive(Parser)]
#[command(version = "0.2.0", about = "Yuchi CLI - A command-line assistant powered by ShapesAI")]
struct Cli {
    /// Path to an image file (PNG/JPEG) to send to the AI
    #[arg(long, value_name = "IMAGE_PATH")]
    image: Option<String>,

    /// Override the model for this question
    #[arg(long, value_name = "MODEL")]
    model: Option<String>,

    /// Reset the AI conversation history (sends '!reset' to AI)
    #[arg(long)]
    reset: bool,

    /// Clear the AI's short-term memory (sends '!wack' to AI)
    #[arg(long)]
    wack: bool,

    /// Save the current conversation state
    #[arg(long)]
    sleep: bool,

    /// Authenticate with ShapesAI
    #[arg(long)]
    login: bool,

    /// Clear stored credentials and configuration
    #[arg(long)]
    logout: bool,

    /// Set a ShapesAI username to use a custom model (shapesinc/<username>)
    #[arg(long, value_name = "USERNAME")]
    shape: Option<String>,

    /// Generate an image and download it (appends '!imagine' to the prompt)
    #[arg(long)]
    imagine: bool,

    /// Question to ask
    #[arg(value_name = "QUESTION")]
    question: Vec<String>,
}

fn main() {
    if let Err(e) = run() {
        display_error(&e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), YuchiError> {
    let cli = Cli::parse();

    // Handle non-AI flags
    if cli.login {
        commands::login()?;
        return Ok(());
    }
    if cli.logout {
        commands::logout()?;
        return Ok(());
    }
    if let Some(username) = cli.shape {
        commands::set_shape(&username)?;
        return Ok(());
    }
    if cli.sleep {
        println!("Saving conversation state...");
        return Ok(());
    }

    // Handle AI-related flags and question
    let prompt = if !cli.question.is_empty() {
        cli.question.join(" ")
    } else {
        String::new()
    };

    if cli.imagine {
        let final_prompt = if prompt.is_empty() {
            "!imagine".to_string()
        } else {
            format!("{} !imagine", prompt)
        };
        let response = commands::ask(&final_prompt, cli.model.as_deref(), cli.image.as_deref())?;
        commands::download_image(&response)?;
    } else if cli.reset {
        commands::ask("!reset", cli.model.as_deref(), None)?;
    } else if cli.wack {
        commands::ask("!wack", cli.model.as_deref(), None)?;
    } else if !prompt.is_empty() {
        commands::ask(&prompt, cli.model.as_deref(), cli.image.as_deref())?;
    } else {
        display_help();
    }

    Ok(())
}
