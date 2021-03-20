
use sqlx::{error::Error as SqlxError};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("database error")]
    DatabaseError(SqlxError),

    #[error("io error")]
    IoError(std::io::Error),
}

impl From<SqlxError> for Error {
    fn from(err: SqlxError) -> Self {
        Error::DatabaseError(err)
    }
}
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::IoError(err)
    }
}