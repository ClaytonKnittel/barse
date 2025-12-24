use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub struct BarseError {
  message: String,
}

impl BarseError {
  pub fn new(message: String) -> Self {
    BarseError { message }
  }
}

impl Error for BarseError {}

impl Display for BarseError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "error: {}", self.message)
  }
}

pub type BarseResult<T = ()> = Result<T, Box<dyn Error + Send + Sync + 'static>>;
