use std::fmt::Display;
use std::fmt::Formatter;

#[derive(Debug)]
pub struct MyError {
    err: anyhow::Error,
}

impl Display for MyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.err.fmt(f)
    }
}

impl actix_web::error::ResponseError for MyError { }

impl From<std::io::Error> for MyError {
    fn from(err: std::io::Error) -> Self {
        MyError { err: anyhow::Error::from(err) }
    }
}

impl From<r2d2::Error> for MyError {
    fn from(err: r2d2::Error) -> Self {
        MyError { err: anyhow::Error::from(err) }
    }
}

impl From<anyhow::Error> for MyError {
    fn from(err: anyhow::Error) -> MyError {
        MyError { err }
    }
}

impl From<rusqlite::Error> for MyError {
    fn from(err: rusqlite::Error) -> MyError {
        MyError { err: anyhow::Error::from(err) }
    }
}
