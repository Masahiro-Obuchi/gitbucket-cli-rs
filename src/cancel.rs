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
