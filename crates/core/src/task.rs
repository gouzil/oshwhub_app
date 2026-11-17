//! Keep CPU preparation off the native reactor; the browser uses its local executor.

pub(crate) struct ComputeError;

pub(crate) async fn compute<T: Send + 'static>(
    job: impl FnOnce() -> T + Send + 'static,
) -> std::result::Result<T, ComputeError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        tokio::task::spawn_blocking(job)
            .await
            .map_err(|_| ComputeError)
    }
    #[cfg(target_arch = "wasm32")]
    {
        // shortcut: browser preparation uses its executor; large-board latency must pass the migration gate before switching.
        Ok(job())
    }
}
