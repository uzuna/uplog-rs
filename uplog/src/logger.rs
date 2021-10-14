/// crate logとintarfaceを近づける実装
use std::{
    error,
    fmt::{self, Display},
};

use crate::{Metadata, Record};

pub trait Log: Sync + Send {
    fn enabled(&self, metadata: &Metadata) -> bool;
    fn log(&self, record: &Record);
    fn flush(&self);
}

struct NopLogger;

impl Log for NopLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        false
    }

    fn log(&self, _: &Record) {}
    fn flush(&self) {}
}

// global logger
static mut LOGGER: &dyn Log = &NopLogger;

pub fn set_boxed_logger(logger: Box<dyn Log>) -> Result<(), SetLoggerError> {
    set_logger_inner(|| Box::leak(logger))
}

fn set_logger_inner<F>(make_logger: F) -> Result<(), SetLoggerError>
where
    F: FnOnce() -> &'static dyn Log,
{
    unsafe {
        LOGGER = make_logger();
    }
    Ok(())
}

#[derive(Debug)]
pub struct SetLoggerError;

impl Display for SetLoggerError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str("already initialized")
    }
}
impl error::Error for SetLoggerError {}

pub fn logger() -> &'static dyn Log {
    unsafe { LOGGER }
}

pub fn flush() {
    unsafe { LOGGER.flush() }
}
