/// crate logとintarfaceを近づける実装
use std::{
    cell::Cell,
    error,
    fmt::{self, Display},
    thread::JoinHandle,
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
static mut HANDLE: Cell<Option<JoinHandle<()>>> = Cell::new(None);

pub fn set_boxed_logger(
    logger: Box<dyn Log>,
    handle: JoinHandle<()>,
) -> Result<(), SetLoggerError> {
    set_therad_handle(handle)?;
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

pub(crate) fn set_therad_handle(handle: JoinHandle<()>) -> Result<(), SetLoggerError> {
    unsafe {
        let glocal_handle = HANDLE.get_mut();
        match glocal_handle {
            Some(_) => Err(SetLoggerError),
            None => {
                *glocal_handle = Some(handle);
                Ok(())
            }
        }
    }
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
    unsafe {
        LOGGER.flush();
        let glocal_handle = HANDLE.get_mut();
        glocal_handle.take().unwrap().join().ok();
        // match glocal_handle {
        //     Some(handle) => {handle.join().ok();}
        //     _ => {},
        // };
    }
}
