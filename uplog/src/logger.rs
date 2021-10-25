/// crate logとintarfaceを近づける実装
use std::{
    cell::Cell,
    error,
    fmt::{self, Display},
    thread::JoinHandle,
};

use crate::{MetadataBorrow, RecordBorrow};

pub trait Log: Sync + Send {
    fn enabled(&self, metadata: &MetadataBorrow) -> bool;
    fn log(&self, record: &RecordBorrow);
    fn flush(&self);
}

struct NopLogger;

impl Log for NopLogger {
    fn enabled(&self, _: &MetadataBorrow) -> bool {
        false
    }

    fn log(&self, _: &RecordBorrow) {}
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

/// flush swapbuffer and closing sender thread
///
/// It is highly recommended to call it before the end of the program
/// to completely send the data in the buffer.
pub fn flush() {
    unsafe {
        LOGGER.flush();
        let glocal_handle = HANDLE.get_mut();
        if let Some(x) = glocal_handle.take() {
            x.join().ok();
        }
    }
}
