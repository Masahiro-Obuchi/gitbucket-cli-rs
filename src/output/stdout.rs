//! Keep print-style callers compatible while reporting I/O failures at dispatch.
use crate::error::{GbError, Result};
use std::{
    fmt,
    io::{self, Read, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

static CLOSED: AtomicBool = AtomicBool::new(false);
static FAILURE: Mutex<Option<io::Error>> = Mutex::new(None);

fn written(result: io::Result<()>) -> Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {
            CLOSED.store(true, Ordering::Relaxed);
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

pub fn write_stdout(bytes: &[u8]) -> Result<()> {
    if CLOSED.load(Ordering::Relaxed) {
        return Ok(());
    }
    let mut stdout = io::stdout().lock();
    written(stdout.write_all(bytes).and_then(|_| stdout.flush()))
}

pub(super) fn stream_stdout(content: &mut impl Read) -> Result<()> {
    if CLOSED.load(Ordering::Relaxed) {
        return Ok(());
    }
    let mut stdout = io::stdout().lock();
    written(io::copy(content, &mut stdout).and_then(|_| stdout.flush()))
}

pub fn stdout_write(args: fmt::Arguments<'_>) {
    if CLOSED.load(Ordering::Relaxed) {
        return;
    }
    let mut stdout = io::stdout().lock();
    if let Err(GbError::Io(error)) = written(stdout.write_fmt(args).and_then(|_| stdout.flush())) {
        let mut failure = FAILURE.lock().unwrap_or_else(|e| e.into_inner());
        if failure.is_none() {
            *failure = Some(error);
        }
        CLOSED.store(true, Ordering::Relaxed);
    }
}

pub fn stdout_line(args: fmt::Arguments<'_>) {
    stdout_write(format_args!("{args}\n"));
}

pub fn finish(result: Result<()>) -> Result<()> {
    let error = FAILURE.lock().unwrap_or_else(|e| e.into_inner()).take();
    result.and(match error {
        Some(error) => Err(error.into()),
        None => Ok(()),
    })
}
