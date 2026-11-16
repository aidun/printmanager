use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Datenbankfehler: {0}")]
    Db(#[from] sqlx::Error),
    #[error("Migration fehlgeschlagen: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("Dateifehler: {0}")]
    Io(#[from] std::io::Error),
    #[error("Nicht gefunden: {0}")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

/// Fehler gehen als lesbarer Text an die Oberfläche.
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
