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

#[test]
fn cancelling_cpu_work_stops_the_batch_and_releases_the_bounded_slot() {
    use std::sync::mpsc;
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let next_started = Arc::new(AtomicBool::new(false));
    let next = Arc::clone(&next_started);
    let mut request = Box::pin(async move {
        services::compute(move || {
            started_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            done_tx.send(()).unwrap();
            Ok(())
        })
        .await?;
        services::compute(move || {
            next.store(true, Ordering::SeqCst);
            Ok(())
        })
        .await
    });
    assert!(
        request
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    drop(request);
    // A cancelled waiter must not start when the current operation releases the slot.
    let queued_started = Arc::new(AtomicBool::new(false));
    let queued = Arc::clone(&queued_started);
    let mut waiter = Box::pin(services::compute(move || {
        queued.store(true, Ordering::SeqCst);
        Ok(())
    }));
    assert!(
        waiter
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    pollster::block_on(services::run(async {
        tokio::time::sleep(Duration::from_millis(25)).await;
        Ok(())
    }))
    .unwrap();
    assert!(
        !queued_started.load(Ordering::SeqCst),
        "CPU slots remain occupied until cancelled work finishes"
    );
    drop(waiter);
    release_tx.send(()).unwrap();
    done_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    // Finishing another operation proves the semaphore was returned, without a timing-only wait.
    pollster::block_on(services::compute(|| Ok(()))).unwrap();
    assert!(!next_started.load(Ordering::SeqCst));
    assert!(!queued_started.load(Ordering::SeqCst));
}
