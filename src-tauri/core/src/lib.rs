mod data;
mod models;
mod reminders;
mod store;
mod tracking;

pub use models::*;
pub use reminders::*;
pub use store::Store;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("SQLite could not complete the operation: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("File operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Stored settings could not be read: {0}")]
    Json(#[from] serde_json::Error),
    #[error("CSV export failed: {0}")]
    Csv(#[from] csv::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}

#[cfg(test)]
mod tests;
