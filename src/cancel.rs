use std::sync::atomic::{AtomicBool, Ordering};

static CANCELLED: AtomicBool = AtomicBool::new(false);
static NOTIFY: tokio::sync::Notify = tokio::sync::Notify::const_new();

pub fn requested() -> bool {
    CANCELLED.load(Ordering::SeqCst)
}

pub async fn notified() {
    loop {
        let notified = NOTIFY.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if requested() {
            return;
        }
        notified.await;
    }
}

/// Keep blocking terminal/stdin reads off the command task so signals can
/// return a cancellation error even when no input ever arrives. The CLI exits
/// on that error; Tokio cannot abort a blocking terminal read itself.
pub async fn input<T, E>(
    read: impl FnOnce() -> std::result::Result<T, E> + Send + 'static,
) -> crate::error::Result<T>
where
    T: Send + 'static,
    E: Into<crate::error::GbError> + Send + 'static,
{
    if requested() {
        return Err(crate::error::GbError::Cancelled);
    }
    let read = tokio::task::spawn_blocking(move || {
        read().map_err(|error| match error.into() {
            // console raises SIGINT for a Ctrl+C key and returns Interrupted.
            // That read can finish before the async signal task is scheduled.
            crate::error::GbError::Dialoguer(dialoguer::Error::IO(error))
                if error.kind() == std::io::ErrorKind::Interrupted =>
            {
                crate::error::GbError::Cancelled
            }
            error => error,
        })
    });
    tokio::select! {
        biased;
        _ = notified() => Err(crate::error::GbError::Cancelled),
        result = read => result.map_err(|error| crate::error::GbError::Other(format!("Input task failed: {error}")))?,
    }
}

pub fn install() -> std::io::Result<()> {
    // Construct synchronously so interception is installed before spawning Git.
    #[cfg(unix)]
    let mut signal = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    #[cfg(windows)]
    let mut signal = tokio::signal::windows::ctrl_c()?;
    #[cfg(windows)]
    let mut terminate = tokio::signal::windows::ctrl_break()?;
    tokio::spawn(async move {
        tokio::select! { _ = signal.recv() => {}, _ = terminate.recv() => {} }
        CANCELLED.store(true, Ordering::SeqCst);
        NOTIFY.notify_waiters();
    });
    Ok(())
}
