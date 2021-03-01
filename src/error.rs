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

impl actix_web::error::ResponseError for MSError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self.err.downcast_ref::<EntityNotFoundError>() {
            Some(r) => r.status_code(),
            _ => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

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


#[derive(Debug)]
pub struct EntityNotFoundError {
    msg: String
}

impl EntityNotFoundError {
    pub fn new(msg: &str) -> Self {
        Self { msg: msg.to_owned() }
    }
}

impl Display for EntityNotFoundError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str(&self.msg)
    }
}

impl actix_web::error::ResponseError for EntityNotFoundError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        actix_web::http::StatusCode::NOT_FOUND
    }
}

impl std::error::Error for EntityNotFoundError {}
