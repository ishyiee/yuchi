use crate::errors::YuchiError;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use prettytable::{Table, Row, Cell};
use std::time::Duration;

pub fn display_response(command: &str, response: &str) {
    println!("{}", "=== Yuchi CLI v0.1.0 ===".bold().cyan());
    println!("{} {}", "Command:".bold(), command);
    println!("{} {}", "Response:".bold(), response.cyan());
    println!();
}

pub fn display_error(e: &YuchiError) {
    println!("{}", "[ERROR]".red().bold());
    match e {
        YuchiError::Config(msg) => {
            println!("{} Configuration", "Type:".bold());
            println!("{} {}", "Message:".bold(), msg);
            println!("{} Run `yuchi --login` to set up credentials", "Suggestion:".bold());
        }
        YuchiError::Api(msg) => {
            println!("{} ShapesAI API", "Type:".bold());
            println!("{} {}", "Message:".bold(), msg);
            println!("{} Check API key or network connection", "Suggestion:".bold());
        }
        YuchiError::Tool(msg) => {
            println!("{} Command Execution", "Type:".bold());
            println!("{} {}", "Message:".bold(), msg);
            println!("{} Verify the command syntax", "Suggestion:".bold());
        }
        YuchiError::Input(msg) => {
            println!("{} User Input", "Type:".bold());
            println!("{} {}", "Message:".bold(), msg);
            println!("{} Provide valid input", "Suggestion:".bold());
        }
        YuchiError::Image(msg) => {
            println!("{} Image Processing", "Type:".bold());
            println!("{} {}", "Message:".bold(), msg);
            println!("{} Check the image path and format (PNG/JPEG)", "Suggestion:".bold());
        }
    }
    println!();
}

pub fn display_help() {
    println!("{}", "Error: No command or question provided.".magenta());
    println!("Run `yuchi --help` for usage information.");
}

pub fn display_progress() -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.cyan} {msg}").expect("Failed to set progress style")
    );
    pb.set_message("Processing...");
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

pub fn display_command_result(command: &str, result: &str) {
    let mut table = Table::new();
    table.add_row(Row::new(vec![
        Cell::new("Command").style_spec("b"),
        Cell::new("Result").style_spec("b"),
    ]));
    table.add_row(Row::new(vec![
        Cell::new(command),
        Cell::new(result),
    ]));
    table.printstd();
}