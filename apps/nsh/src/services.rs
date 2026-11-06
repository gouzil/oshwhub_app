use std::future::Future;

use oshwhub_core::error::Result;

pub fn client() -> Result<oshwhub_core::client::HttpClient> {
    #[cfg(target_arch = "wasm32")]
    {
        Ok(oshwhub_core::client::HttpClient::via_local_service())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        static CLIENT: std::sync::OnceLock<Result<oshwhub_core::client::HttpClient>> =
            std::sync::OnceLock::new();
        CLIENT
            .get_or_init(oshwhub_core::client::HttpClient::new)
            .clone()
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn run<T>(job: impl Future<Output = Result<T>>) -> Result<T> {
    job.await
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn run<T: Send + 'static>(
    job: impl Future<Output = Result<T>> + Send + 'static,
) -> Result<T> {
    use oshwhub_core::error::AppError;
    use std::sync::OnceLock;

    static RUNTIME: OnceLock<std::result::Result<tokio::runtime::Runtime, std::io::Error>> =
        OnceLock::new();
    let runtime = RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("oshwhub-core")
                .enable_all()
                .build()
        })
        .as_ref()
        .map_err(|_| AppError::new("internal", "无法启动 Rust 核心运行时"))?;

    struct CancelOnDrop(tokio::task::AbortHandle);
    impl Drop for CancelOnDrop {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let job = runtime.spawn(job);
    let _cancel = CancelOnDrop(job.abort_handle());
    job.await
        .map_err(|_| AppError::new("internal", "Rust 核心任务未能完成"))?
}
