use std::fmt::Display;
use std::fmt::Formatter;

// MusicSyncError for lack of a better name
#[derive(Debug)]
pub struct MSError {
    err: anyhow::Error,
}

impl Display for MSError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.err.fmt(f)
    }
}

impl actix_web::error::ResponseError for MSError { }

impl From<std::io::Error> for MSError {
    fn from(err: std::io::Error) -> Self {
        MSError { err: anyhow::Error::from(err) }
    }
}

impl From<r2d2::Error> for MSError {
    fn from(err: r2d2::Error) -> Self {
        MSError { err: anyhow::Error::from(err) }
    }
}

impl From<anyhow::Error> for MSError {
    fn from(err: anyhow::Error) -> MSError {
        MSError { err }
    }
}

impl From<rusqlite::Error> for MSError {
    fn from(err: rusqlite::Error) -> MSError {
        MSError { err: anyhow::Error::from(err) }
    }
}
