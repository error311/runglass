use std::io::{self, Read};
use std::sync::{
    mpsc::{self, Receiver},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;
use tiny_http::{Header, Response, StatusCode};

use super::{run_job_payload, JobStore, RunJobState};

pub(crate) fn run_job_event_stream_response(
    jobs: &JobStore,
    job_id: &str,
) -> Response<ChannelReader> {
    let (sender, receiver) = mpsc::channel::<Vec<u8>>();
    let jobs = Arc::clone(jobs);
    let job_id = job_id.to_string();
    thread::spawn(move || {
        let _ = sender.send(b"retry: 1000\n\n".to_vec());
        let mut last_payload = None::<String>;
        let mut last_emit = None::<Instant>;
        let mut last_heartbeat = Instant::now();

        loop {
            let snapshot = jobs
                .lock()
                .ok()
                .and_then(|store| store.get(&job_id).cloned());
            let Some(job) = snapshot else {
                let _ = sender.send(format_sse_payload(
                    "error",
                    &json!({ "error": "run job not found" }).to_string(),
                ));
                break;
            };

            let payload = run_job_payload(&job).to_string();
            let terminal = !matches!(job.state, RunJobState::Running);
            let ready_for_emit = terminal
                || last_emit
                    .map(|at| at.elapsed() >= Duration::from_millis(350))
                    .unwrap_or(true);
            if last_payload.as_ref() != Some(&payload) && ready_for_emit {
                if sender.send(format_sse_payload("job", &payload)).is_err() {
                    break;
                }
                last_payload = Some(payload);
                last_emit = Some(Instant::now());
                last_heartbeat = Instant::now();
            } else if last_heartbeat.elapsed() >= Duration::from_secs(2) {
                if sender.send(b": keepalive\n\n".to_vec()).is_err() {
                    break;
                }
                last_heartbeat = Instant::now();
            }

            if !matches!(job.state, RunJobState::Running) {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
    });

    let headers = vec![
        Header::from_bytes("Content-Type", "text/event-stream; charset=utf-8")
            .expect("valid header"),
        Header::from_bytes("Cache-Control", "no-cache").expect("valid header"),
        Header::from_bytes("X-Accel-Buffering", "no").expect("valid header"),
    ];
    Response::new(
        StatusCode(200),
        headers,
        ChannelReader::new(receiver),
        None,
        None,
    )
    .with_chunked_threshold(0)
}

fn format_sse_payload(event: &str, payload: &str) -> Vec<u8> {
    let mut body = String::new();
    body.push_str("event: ");
    body.push_str(event);
    body.push('\n');
    for line in payload.lines() {
        body.push_str("data: ");
        body.push_str(line);
        body.push('\n');
    }
    body.push('\n');
    body.into_bytes()
}

pub(crate) struct ChannelReader {
    receiver: Receiver<Vec<u8>>,
    buffer: Vec<u8>,
    offset: usize,
}

impl ChannelReader {
    fn new(receiver: Receiver<Vec<u8>>) -> Self {
        Self {
            receiver,
            buffer: Vec::new(),
            offset: 0,
        }
    }
}

impl Read for ChannelReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }

        loop {
            if self.offset < self.buffer.len() {
                let remaining = self.buffer.len() - self.offset;
                let len = remaining.min(out.len());
                out[..len].copy_from_slice(&self.buffer[self.offset..self.offset + len]);
                self.offset += len;
                if self.offset >= self.buffer.len() {
                    self.buffer.clear();
                    self.offset = 0;
                }
                return Ok(len);
            }

            match self.receiver.recv() {
                Ok(chunk) => {
                    self.buffer = chunk;
                    self.offset = 0;
                }
                Err(_) => return Ok(0),
            }
        }
    }
}
