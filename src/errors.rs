use std::fmt;

#[derive(Debug)]
pub enum YuchiError {
    Api(String),
    Config(String),
    Input(String),
    Image(String),
    Tool(String),
}

impl fmt::Display for YuchiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            YuchiError::Api(msg) => write!(f, "API Error: {}", msg),
            YuchiError::Config(msg) => write!(f, "Config Error: {}", msg),
            YuchiError::Input(msg) => write!(f, "Input Error: {}", msg),
            YuchiError::Image(msg) => write!(f, "Image Error: {}", msg),
            YuchiError::Tool(msg) => write!(f, "Tool Error: {}", msg),
        }
    }
}

impl std::error::Error for YuchiError {}