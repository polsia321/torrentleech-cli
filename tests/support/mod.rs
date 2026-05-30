#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Debug)]
pub struct MockServer {
    addr: SocketAddr,
    state: Arc<Mutex<ServerState>>,
    handle: Option<JoinHandle<()>>,
}

impl MockServer {
    pub fn start() -> Self {
        let listener = bind_available_listener();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let state = Arc::new(Mutex::new(ServerState::default()));
        let thread_state = Arc::clone(&state);
        let handle = thread::spawn(move || serve(listener, thread_state));
        thread::sleep(Duration::from_millis(25));
        Self {
            addr,
            state,
            handle: Some(handle),
        }
    }

    pub fn uri(&self) -> String {
        format!("http://{}", self.addr)
    }
}

fn bind_available_listener() -> TcpListener {
    static NEXT_PORT: AtomicU16 = AtomicU16::new(20_000);

    for _ in 20_000..60_000 {
        let port = NEXT_PORT.fetch_add(1, Ordering::Relaxed);
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) {
            return listener;
        }
    }
    TcpListener::bind("127.0.0.1:0").unwrap()
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.state.lock().unwrap().shutdown = true;
        let _ = TcpStream::connect(self.addr);
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
        let state = self.state.lock().unwrap();
        if !std::thread::panicking() {
            for expectation in &state.expectations {
                let actual = expectation.seen;
                if let Some(expected) = expectation.expected {
                    assert_eq!(actual, expected, "fixture request count mismatch");
                } else if let Some(max) = expectation.max {
                    assert!(actual <= max, "fixture request count exceeded");
                }
            }
        }
    }
}

#[derive(Debug, Default)]
struct ServerState {
    expectations: Vec<Expectation>,
    shutdown: bool,
}

#[derive(Debug)]
struct Expectation {
    matchers: Vec<Matcher>,
    response: ResponseTemplate,
    expected: Option<u64>,
    max: Option<u64>,
    seen: u64,
}

#[derive(Debug, Clone)]
pub struct Mock {
    matchers: Vec<Matcher>,
    response: ResponseTemplate,
    expected: Option<u64>,
    max: Option<u64>,
}

impl Mock {
    pub fn given(matcher: Matcher) -> Self {
        Self {
            matchers: vec![matcher],
            response: ResponseTemplate::new(200),
            expected: None,
            max: None,
        }
    }

    pub fn and(mut self, matcher: Matcher) -> Self {
        self.matchers.push(matcher);
        self
    }

    pub fn respond_with(mut self, response: ResponseTemplate) -> Self {
        self.response = response;
        self
    }

    pub fn expect(mut self, count: u64) -> Self {
        self.expected = Some(count);
        self
    }

    pub fn up_to_n_times(mut self, count: u64) -> Self {
        self.max = Some(count);
        self
    }

    pub fn mount(self, server: &MockServer) {
        server.state.lock().unwrap().expectations.push(Expectation {
            matchers: self.matchers,
            response: self.response,
            expected: self.expected,
            max: self.max,
            seen: 0,
        });
    }
}

#[derive(Debug, Clone)]
pub struct ResponseTemplate {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl ResponseTemplate {
    pub fn new(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn set_body_string(mut self, body: impl Into<String>) -> Self {
        self.body = body.into().into_bytes();
        self
    }

    pub fn set_body_bytes(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    pub fn append_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

#[derive(Debug, Clone)]
pub enum Matcher {
    Method(String),
    Path(String),
    Header(String, String),
    HeaderExact(String, String),
    BodyContains(String),
}

pub mod matchers {
    use super::Matcher;

    pub fn method(value: impl Into<String>) -> Matcher {
        Matcher::Method(value.into())
    }

    pub fn path(value: impl Into<String>) -> Matcher {
        Matcher::Path(value.into())
    }

    pub fn header(name: impl Into<String>, value: impl Into<String>) -> Matcher {
        Matcher::Header(name.into(), value.into())
    }

    pub fn header_exact(name: impl Into<String>, value: impl Into<String>) -> Matcher {
        Matcher::HeaderExact(name.into(), value.into())
    }

    pub fn body_string_contains(value: impl Into<String>) -> Matcher {
        Matcher::BodyContains(value.into())
    }
}

#[derive(Debug)]
struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Matcher {
    fn matches(&self, request: &Request) -> bool {
        match self {
            Self::Method(expected) => request.method.eq_ignore_ascii_case(expected),
            Self::Path(expected) => request.path == *expected,
            Self::Header(name, expected) => request.headers.iter().any(|(actual_name, value)| {
                actual_name.eq_ignore_ascii_case(name) && value.contains(expected)
            }),
            Self::HeaderExact(name, expected) => {
                request.headers.iter().any(|(actual_name, value)| {
                    actual_name.eq_ignore_ascii_case(name) && value == expected
                })
            }
            Self::BodyContains(expected) => {
                String::from_utf8_lossy(&request.body).contains(expected)
            }
        }
    }
}

fn serve(listener: TcpListener, state: Arc<Mutex<ServerState>>) {
    loop {
        match listener.accept() {
            Ok((stream, _)) => handle_stream(stream, &state),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if state.lock().unwrap().shutdown {
                    break;
                }
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("fixture accept failed: {error}"),
        }
    }
}

fn handle_stream(mut stream: TcpStream, state: &Arc<Mutex<ServerState>>) {
    let Some(request) = read_request(&mut stream) else {
        return;
    };
    let response = {
        let mut state = state.lock().unwrap();
        let expectation = state
            .expectations
            .iter_mut()
            .find(|expectation| {
                expectation.expected != Some(expectation.seen)
                    && expectation.max.is_none_or(|max| expectation.seen < max)
                    && expectation
                        .matchers
                        .iter()
                        .all(|matcher| matcher.matches(&request))
            })
            .unwrap_or_else(|| panic!("unexpected fixture request: {request:?}"));
        expectation.seen += 1;
        expectation.response.clone()
    };
    write_response(&mut stream, &response);
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    stream.set_nonblocking(false).unwrap();
    let mut reader = BufReader::new(stream);
    let mut first_line = String::new();
    if reader.read_line(&mut first_line).ok()? == 0 {
        return None;
    }
    let mut parts = first_line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let mut headers = Vec::new();
    let mut content_length = 0;
    let mut chunked = false;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim().to_string();
            let value = value.trim().to_string();
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.parse().unwrap();
            } else if name.eq_ignore_ascii_case("transfer-encoding")
                && value.eq_ignore_ascii_case("chunked")
            {
                chunked = true;
            }
            headers.push((name, value));
        }
    }
    let body = if chunked {
        read_chunked_body(&mut reader)
    } else {
        let mut body = vec![0; content_length];
        reader.read_exact(&mut body).unwrap();
        body
    };
    Some(Request {
        method,
        path,
        headers,
        body,
    })
}

fn read_chunked_body(reader: &mut BufReader<&mut TcpStream>) -> Vec<u8> {
    let mut body = Vec::new();
    loop {
        let mut size_line = String::new();
        reader.read_line(&mut size_line).unwrap();
        let size_text = size_line
            .trim_end_matches(['\r', '\n'])
            .split_once(';')
            .map_or(size_line.trim_end_matches(['\r', '\n']), |(size, _)| size);
        let size = usize::from_str_radix(size_text, 16).unwrap();
        if size == 0 {
            let mut trailer = String::new();
            loop {
                trailer.clear();
                reader.read_line(&mut trailer).unwrap();
                if trailer.trim_end_matches(['\r', '\n']).is_empty() {
                    break;
                }
            }
            break;
        }
        let start = body.len();
        body.resize(start + size, 0);
        reader.read_exact(&mut body[start..]).unwrap();
        let mut crlf = [0; 2];
        reader.read_exact(&mut crlf).unwrap();
    }
    body
}

fn write_response(stream: &mut TcpStream, response: &ResponseTemplate) {
    let reason = match response.status {
        200 => "OK",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };
    write!(stream, "HTTP/1.1 {} {}\r\n", response.status, reason).unwrap();
    for (name, value) in &response.headers {
        write!(stream, "{name}: {value}\r\n").unwrap();
    }
    write!(
        stream,
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        response.body.len()
    )
    .unwrap();
    stream.write_all(&response.body).unwrap();
}
