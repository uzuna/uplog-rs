use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("connection error")]
    Connection(#[from] tungstenite::Error),
    #[error("io error")]
    Io(#[from] std::io::Error),
}

pub(crate) const ERROR_MESSAGE_MUTEX_LOCK: &str = "failed to lock mutex";
