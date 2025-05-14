mod api;
mod ui;
mod config;
mod commands;
mod errors;

use clap::{Parser, Subcommand};
use errors::YuchiError;

#[derive(Parser)]
#[command(
    name = "yuchi",
    about = "A CLI assistant powered by ShapesAI",
    version = "0.2.0",
    after_help = "To authenticate, run `yuchi --login` and choose:\n\
                  - Option 1: Enter your ShapesAI API key.\n\
                  - Option 2: Use the built-in App ID, visit the authorization URL, and paste the one-time code.\n\
                  Credentials are stored securely in a platform-appropriate config directory.\n\n\
                  Core Commands:\n\
                  - `--login`: Authenticate with ShapesAI\n\
                  - `--image <PATH>`: Process an image (standalone or with question)\n\
                  - `--reset`: Reset conversation history\n\
                  - `--wack`: Clear short-term memory\n\
                  - `--sleep`: Save conversation state\n\
                  - `run <COMMAND>`: Execute a shell command\n\n\
                  Examples:\n\
                  yuchi \"What's the weather?\"\n\
                  yuchi --image meme.jpg \"What's the text?\"\n\
                  yuchi run ls"
)]
struct Cli {
    /// Log in by setting an API key or user auth token
    #[arg(long)]
    login: bool,
    /// Set a ShapesAI username to use a custom model (shapesinc/<username>)
    #[arg(long, value_name = "USERNAME")]
    shape: Option<String>,
    /// Clear stored credentials and configuration
    #[arg(long)]
    logout: bool,
    /// Reset the conversation history
    #[arg(long)]
    reset: bool,
    /// Clear short-term memory
    #[arg(long)]
    wack: bool,
    /// Save the current conversation state
    #[arg(long)]
    sleep: bool,
    /// Override the model for this question
    #[arg(long, value_name = "MODEL")]
    model: Option<String>,
    /// Path to an image file
    #[arg(long, value_name = "IMAGE_PATH")]
    image: Option<String>,
    /// Subcommands
    #[command(subcommand)]
    command: Option<Commands>,
    /// Prompt for the assistant (if no subcommand)
    #[arg(value_name = "QUESTION", trailing_var_arg = true, allow_hyphen_values = true)]
    question: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a shell command
    Run {
        /// The shell command to execute
        #[arg(value_name = "COMMAND")]
        command: Vec<String>,
    },
}

fn main() {
    if let Err(e) = run() {
        ui::display_error(&e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), YuchiError> {
    let cli = Cli::parse();

    if cli.login {
        return commands::login();
    }

    if let Some(username) = cli.shape {
        return commands::set_shape(&username);
    }

    if cli.logout {
        return commands::logout();
    }

    if cli.reset {
        return commands::ask("!reset", cli.model.as_deref(), None);
    }

    if cli.wack {
        return commands::ask("!wack", cli.model.as_deref(), None);
    }

    if cli.sleep {
        return commands::ask("!sleep", cli.model.as_deref(), None);
    }

    if let Some(command) = cli.command {
        match command {
            Commands::Run { command } => {
                let command_str = command.join(" ");
                return commands::run_tool(&command_str);
            }
        }
    }

    if cli.image.is_some() && cli.question.is_empty() {
        return commands::ask("Describe this image", cli.model.as_deref(), cli.image.as_deref());
    }

    if !cli.question.is_empty() {
        let question = cli.question.join(" ");
        return commands::ask(&question, cli.model.as_deref(), cli.image.as_deref());
    }

    ui::display_help();
    Ok(())
}