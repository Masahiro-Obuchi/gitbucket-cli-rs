#![allow(dead_code)]
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

const SERVER_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct CapturedRequest {
    pub method: String,
    pub target: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

#[derive(Debug)]
pub struct ScriptedResponse {
    pub expected_request_line: String,
    pub status_line: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl ScriptedResponse {
    pub fn json(expected_request_line: &str, status_line: &str, body: &str) -> Self {
        Self {
            expected_request_line: expected_request_line.into(),
            status_line: status_line.into(),
            headers: vec![("content-type".into(), "application/json".into())],
            body: body.into(),
        }
    }

    pub fn html(expected_request_line: &str, status_line: &str, body: &str) -> Self {
        Self {
            expected_request_line: expected_request_line.into(),
            status_line: status_line.into(),
            headers: vec![("content-type".into(), "text/html; charset=utf-8".into())],
            body: body.into(),
        }
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

fn accept_with_timeout(listener: &TcpListener) -> TcpStream {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + SERVER_TIMEOUT;

    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_read_timeout(Some(SERVER_TIMEOUT)).unwrap();
                stream.set_write_timeout(Some(SERVER_TIMEOUT)).unwrap();
                return stream;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    panic!("timed out waiting for CLI to connect to mock server");
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => panic!("failed to accept mock server connection: {err}"),
        }
    }
}

fn read_request(stream: &mut TcpStream) -> CapturedRequest {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let header_end;
    loop {
        let read = match stream.read(&mut chunk) {
            Ok(read) => read,
            Err(err)
                if err.kind() == io::ErrorKind::TimedOut
                    || err.kind() == io::ErrorKind::WouldBlock =>
            {
                panic!("timed out while reading request headers from CLI");
            }
            Err(err) => panic!("failed to read request headers from CLI: {err}"),
        };
        if read == 0 {
            panic!("connection closed before request headers were fully read");
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
    }

    let header_text = String::from_utf8(buffer[..header_end].to_vec()).unwrap();
    let mut lines = header_text.split("\r\n").filter(|line| !line.is_empty());
    let request_line = lines.next().unwrap();
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap().to_string();
    let target = request_parts.next().unwrap().to_string();

    let mut headers = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    while buffer.len() < header_end + content_length {
        let read = match stream.read(&mut chunk) {
            Ok(read) => read,
            Err(err)
                if err.kind() == io::ErrorKind::TimedOut
                    || err.kind() == io::ErrorKind::WouldBlock =>
            {
                panic!("timed out while reading request body from CLI");
            }
            Err(err) => panic!("failed to read request body from CLI: {err}"),
        };
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    let body_bytes =
        &buffer[header_end..header_end + content_length.min(buffer.len() - header_end)];
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();

    CapturedRequest {
        method,
        target,
        headers,
        body,
    }
}

fn write_response(
    stream: &mut TcpStream,
    status_line: &str,
    headers: &[(String, String)],
    body: &str,
) {
    let mut raw_headers = String::new();
    for (name, value) in headers {
        raw_headers.push_str(name);
        raw_headers.push_str(": ");
        raw_headers.push_str(value);
        raw_headers.push_str("\r\n");
    }

    let response = format!(
        "HTTP/1.1 {}\r\n{}content-length: {}\r\nconnection: close\r\n\r\n{}",
        status_line,
        raw_headers,
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).unwrap();
}

pub fn spawn_server(status_line: &str, body: &str) -> (u16, thread::JoinHandle<CapturedRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let status_line = status_line.to_string();
    let body = body.to_string();

    let handle = thread::spawn(move || {
        let mut stream = accept_with_timeout(&listener);
        let request = read_request(&mut stream);
        write_response(
            &mut stream,
            &status_line,
            &[("content-type".into(), "application/json".into())],
            &body,
        );
        request
    });

    (port, handle)
}

pub fn spawn_scripted_server(
    responses: Vec<ScriptedResponse>,
) -> (u16, thread::JoinHandle<Vec<CapturedRequest>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let handle = thread::spawn(move || {
        let mut captured = Vec::new();
        for response in responses {
            let mut stream = accept_with_timeout(&listener);
            let request = read_request(&mut stream);
            let request_line = format!("{} {} HTTP/1.1", request.method, request.target);
            assert_eq!(request_line, response.expected_request_line);
            write_response(
                &mut stream,
                &response.status_line,
                &response.headers,
                &response.body,
            );
            captured.push(request);
        }
        captured
    });

    (port, handle)
}
