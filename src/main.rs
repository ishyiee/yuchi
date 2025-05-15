use clap::Parser;
use crate::commands::Commands;
use crate::errors::YuchiError;
use crate::ui::{display_error, display_help};

#[derive(Parser)]
#[command(version = "0.2.0", about = "Yuchi CLI - A command-line assistant powered by ShapesAI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to an image file (PNG/JPEG)
    #[arg(long, value_name = "IMAGE_PATH")]
    image: Option<String>,

    /// Override the model for this question
    #[arg(long, value_name = "MODEL")]
    model: Option<String>,

    /// Reset the conversation history
    #[arg(long)]
    reset: bool,

    /// Clear short-term memory
    #[arg(long)]
    wack: bool,

    /// Save the current conversation state
    #[arg(long)]
    sleep: bool,

    /// Question to ask
    #[arg(value_name = "QUESTION")]
    question: Option<String>,
}

fn main() {
    if let Err(e) = run() {
        display_error(&e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), YuchiError> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Login) => {
            Commands::login()?;
        }
        Some(Commands::Shape { username }) => {
            Commands::set_shape(&username)?;
        }
        Some(Commands::Logout) => {
            Commands::logout()?;
        }
        Some(Commands::Run { command }) => {
            let command_str = command.join(" ");
            let (_result, _success) = Commands::run_tool(&command_str, None)?;
            return Ok(());
        }
        None => {
            if cli.reset {
                println!("Resetting conversation history...");
                return Ok(());
            }
            if cli.wack {
                println!("Clearing short-term memory...");
                return Ok(());
            }
            if cli.sleep {
                println!("Saving conversation state...");
                return Ok(());
            }

            if let Some(question) = cli.question {
                Commands::ask(&question, cli.model.as_deref(), cli.image.as_deref())?;
            } else {
                display_help();
            }
        }
    }

    Ok(())
}