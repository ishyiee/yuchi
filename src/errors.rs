use thiserror::Error;

#[derive(Error, Debug)]
pub enum YuchiError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("API error: {0}")]
    Api(String),
    #[error("Tool execution error: {0}")]
    Tool(String),
    #[error("Invalid input: {0}")]
    Input(String),
    #[error("Image processing error: {0}")]
    Image(String),
}