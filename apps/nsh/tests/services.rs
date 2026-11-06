#![cfg(not(target_arch = "wasm32"))]

use std::{
    future::{Future, pending},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};

use oshwhub_core::{api::Api, client::HttpClient, error::Result};
use oshwhub_nsh::services;

#[test]
fn native_jobs_have_a_reactor_and_keep_core_errors_typed() {
    let response = pollster::block_on(services::run(async {
        tokio::time::sleep(Duration::from_millis(1)).await;
        Api::new(HttpClient::new()?).project("../invalid").await
    }));
    assert_eq!(response.err().unwrap().code, "INVALID_ID");
}

#[test]
fn disposing_a_request_cancels_its_native_job() {
    struct Dropped(Arc<AtomicBool>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let guard = Dropped(Arc::clone(&dropped));
    let mut request = Box::pin(services::run(async move {
        let _guard = guard;
        pending::<Result<()>>().await
    }));
    assert!(matches!(
        request
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));
    drop(request);
    let deadline = Instant::now() + Duration::from_secs(1);
    while !dropped.load(Ordering::SeqCst) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(dropped.load(Ordering::SeqCst));
}
