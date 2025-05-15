use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use prettytable::{Table, Row, Cell};
use crate::errors::YuchiError;

pub fn display_help() {
    println!("{}", "=== Yuchi CLI v0.2.0 ===".bold().cyan());
    println!("A command-line assistant powered by ShapesAI.");
    println!("\nUsage: yuchi [OPTIONS] [QUESTION...]");
    println!("\nOptions:");
    println!("  --login                  Authenticate with ShapesAI (API key or user auth token)");
    println!("  --shape <USERNAME>       Set a ShapesAI username to use a custom model (shapesinc/<username>)");
    println!("  --logout                 Clear stored credentials and configuration");
    println!("  --reset                  Reset the AI conversation history (sends '!reset' to AI)");
    println!("  --wack                   Clear the AI's short-term memory (sends '!wack' to AI)");
    println!("  --sleep                  Save the current conversation state");
    println!("  --model <MODEL>          Override the model for this question");
    println!("  --image <IMAGE_PATH>     Path to an image file (PNG/JPEG) to send to the AI");
    println!("  --imagine                Generate an image via AI and download it (appends '!imagine' to the prompt)");
    println!("\nNote: Multi-word questions can be entered without quotes (e.g., yuchi hows you)");
    println!("\nExamples:");
    println!("  yuchi hi");
    println!("  yuchi hows you");
    println!("  yuchi --imagine a train station");
    println!("  yuchi --image meme.jpg What's the text?");
    println!("\nRun `yuchi --login` to authenticate first.");
}

pub fn display_error(error: &YuchiError) {
    let error_message = match error {
        YuchiError::Api(msg) => format!("API Error: {}", msg),
        YuchiError::Config(msg) => format!("Config Error: {}", msg),
        YuchiError::Input(msg) => format!("Input Error: {}", msg),
        YuchiError::Image(msg) => format!("Image Error: {}", msg),
        YuchiError::Tool(msg) => format!("Tool Error: {}", msg),
    };
    eprintln!("{}", error_message.red().bold());
}

pub fn display_progress() -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.cyan} {msg}").unwrap()
    );
    pb.set_message("Processing...");
    pb
}

pub fn display_response(_question: &str, response: &str) {
    println!("{}", format!("Yuchi: {}", response).cyan());
}

pub fn display_command_result(command: &str, result: &str) {
    let mut table = Table::new();
    table.add_row(Row::new(vec![
        Cell::new("Command").style_spec("bFc"),
        Cell::new(command).style_spec("c"),
    ]));
    table.add_row(Row::new(vec![
        Cell::new("Result").style_spec("bFc"),
        Cell::new(result).style_spec("c"),
    ]));
    table.printstd();
}
