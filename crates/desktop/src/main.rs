use serde::{Deserialize, Serialize};
use std::{
    io::{self, BufRead, BufReader, BufWriter, Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::{sync::Semaphore, task::JoinSet};

const MAX_INPUT_BYTES: usize = 1024 * 1024;
// JSON escaping may expand one input byte to six envelope bytes.
const MAX_LINE_BYTES: usize = MAX_INPUT_BYTES * 6 + 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: String,
    input: String,
}

#[derive(Serialize)]
struct Response {
    id: String,
    output: String,
}

fn invalid_protocol() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Invalid desktop core frame")
}

fn serve() -> io::Result<()> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--log-dir")) {
        return Err(invalid_protocol());
    }
    let log_dir = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(invalid_protocol)?;
    if args.next().is_some() {
        return Err(invalid_protocol());
    }
    oshwhub_core::diagnostics::initialize(&log_dir)?;
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "desktop.ready");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let _entered = runtime.enter();
    let stdout = Arc::new(Mutex::new(BufWriter::new(io::stdout())));
    let semaphore = Arc::new(Semaphore::new(32));
    let mut tasks = JoinSet::new();
    let mut reader = BufReader::new(io::stdin().lock());
    loop {
        while let Some(result) = tasks.try_join_next() {
            result.map_err(io::Error::other)??;
        }
        let mut line = Vec::new();
        let count = reader
            .by_ref()
            .take((MAX_LINE_BYTES + 1) as u64)
            .read_until(b'\n', &mut line)?;
        if count == 0 {
            break;
        }
        if count > MAX_LINE_BYTES || line.last() != Some(&b'\n') {
            return Err(invalid_protocol());
        }
        let request: Request = serde_json::from_slice(&line).map_err(|_| invalid_protocol())?;
        if request.id.is_empty()
            || request.id.len() > 128
            || !request
                .id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || request.input.len() > MAX_INPUT_BYTES
        {
            return Err(invalid_protocol());
        }
        let permit = runtime
            .block_on(semaphore.clone().acquire_owned())
            .map_err(io::Error::other)?;
        let stdout = stdout.clone();
        tasks.spawn(async move {
            let _permit = permit;
            let response = Response {
                id: request.id,
                output: oshwhub_core::dispatch_json(&request.input).await,
            };
            let mut writer = stdout
                .lock()
                .map_err(|_| io::Error::other("Core output lock poisoned"))?;
            serde_json::to_writer(&mut *writer, &response)?;
            writer.write_all(b"\n")?;
            writer.flush()
        });
    }
    runtime.block_on(async {
        while let Some(result) = tasks.join_next().await {
            result.map_err(io::Error::other)??;
        }
        Ok(())
    })
}

fn main() {
    if serve().is_err() {
        // Do not put request content or log-directory details on the transport.
        eprintln!("Desktop core stopped: transport or diagnostic initialization failed");
        std::process::exit(1);
    }
}
