use serde::{Serialize, Serializer};

#[derive(Debug)]
pub struct AppError(pub String);

pub type Result<T> = std::result::Result<T, AppError>;

impl<E: std::fmt::Display> From<E> for AppError {
    fn from(error: E) -> Self {
        AppError(error.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

pub fn fail<T>(message: impl Into<String>) -> Result<T> {
    Err(AppError(message.into()))
}

pub trait Context<T> {
    fn ctx(self, message: &str) -> Result<T>;
}

impl<T, E: std::fmt::Display> Context<T> for std::result::Result<T, E> {
    fn ctx(self, message: &str) -> Result<T> {
        self.map_err(|e| AppError(format!("{message}: {e}")))
    }
}

impl<T> Context<T> for Option<T> {
    fn ctx(self, message: &str) -> Result<T> {
        self.ok_or_else(|| AppError(message.to_string()))
    }
}
