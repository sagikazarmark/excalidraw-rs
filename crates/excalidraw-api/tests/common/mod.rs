//! A canned HTTP origin on loopback, shared by the blocking and async suites.
//!
//! A real `TcpListener` on 127.0.0.1 covers the transport without a mock-server
//! dependency: the client under test is the shipping one, and the assertions are
//! made against bytes that actually crossed a socket. `Connection: close` on
//! every canned response keeps the accept loop one-request-per-connection, so
//! the recorded order is the request order.
//!
//! Only the origin lives here. Each suite keeps its own `client()` constructor,
//! because that is the one line where the two clients differ — which is the
//! point: with the harness shared, a divergence between
//! `client_loopback.rs` and `client_loopback_async.rs` is a real divergence
//! between the two clients, not a difference in how they are exercised.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::thread::JoinHandle;

/// One request as the origin received it.
#[derive(Clone, Debug)]
pub struct Seen {
    pub start_line: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Seen {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// A canned response: status, headers, body.
pub type Canned = (u16, Vec<(&'static str, &'static str)>, &'static str);

/// Serve `script` in order, then stop accepting.
///
/// Returns the base URL and a handle yielding every request received. The
/// listener is bound before returning, so there is no connect race.
pub fn origin(script: Vec<Canned>) -> (String, JoinHandle<Vec<Seen>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let base = format!("http://{}", listener.local_addr().expect("local addr"));
    listener
        .set_nonblocking(true)
        .expect("nonblocking listener");
    let handle = std::thread::spawn(move || {
        let mut seen = Vec::new();
        for canned in script {
            // A script may offer more responses than the client chooses to ask
            // for; that is exactly what the over-fetch regression asserts. Poll
            // with a deadline so a correct client ending the walk early lets
            // this thread finish instead of blocking `join` forever.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            let accepted = loop {
                match listener.accept() {
                    Ok(pair) => break Some(pair),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() >= deadline {
                            break None;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(2));
                    }
                    Err(_) => break None,
                }
            };
            let Some((mut stream, _)) = accepted else {
                break;
            };
            stream.set_nonblocking(false).expect("blocking stream");
            // A connection can be opened without a request ever being written on
            // it. Time out rather than blocking this thread forever.
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .expect("read timeout");
            let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));

            let mut start_line = String::new();
            match reader.read_line(&mut start_line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let mut headers = Vec::new();
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                let trimmed = line.trim_end();
                if trimmed.is_empty() {
                    break;
                }
                let (name, value) = trimmed.split_once(':').expect("header separator");
                headers.push((name.trim().to_owned(), value.trim().to_owned()));
            }
            let length: usize = headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                .and_then(|(_, v)| v.parse().ok())
                .unwrap_or(0);
            let mut body = vec![0u8; length];
            if reader.read_exact(&mut body).is_err() {
                break;
            }

            seen.push(Seen {
                start_line: start_line.trim_end().to_owned(),
                headers,
                body: String::from_utf8(body).expect("utf-8 body"),
            });

            let (status, extra, payload) = canned;
            let mut response = format!(
                "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n",
                payload.len()
            );
            for (name, value) in extra {
                response.push_str(&format!("{name}: {value}\r\n"));
            }
            response.push_str("\r\n");
            response.push_str(payload);
            if stream.write_all(response.as_bytes()).is_err() {
                break;
            }
            let _ = stream.flush();
        }
        seen
    });
    (base, handle)
}

pub const JSON: (&str, &str) = ("Content-Type", "application/json");

pub fn page(items: &[&str], has_next: bool, offset: u64) -> String {
    let data: Vec<String> = items
        .iter()
        .map(|id| {
            format!(
                r#"{{"id":"{id}","name":"{id}","workspace":"w","created":"2026-01-01T00:00:00.000Z"}}"#
            )
        })
        .collect();
    format!(
        r#"{{"limit":1,"offset":{offset},"hasNextPage":{has_next},"data":[{}]}}"#,
        data.join(",")
    )
}
