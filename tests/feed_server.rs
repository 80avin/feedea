#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::{self, JoinHandle};

pub struct FeedServer {
    pub url: String,
    handle: JoinHandle<()>,
}

impl FeedServer {
    pub fn start(rss_body: String, connections: usize) -> FeedServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}/feed.xml");
        let handle = thread::spawn(move || {
            let mut served = 0;
            let mut last_request: Option<std::time::Instant> = None;
            while served < connections {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        served += 1;
                        last_request = Some(std::time::Instant::now());
                        let _ = stream.set_nonblocking(false);
                        let mut buf = [0u8; 4096];
                        let _ = stream.read(&mut buf);
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            rss_body.len(),
                            rss_body
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if last_request.is_some_and(|t| t.elapsed() > std::time::Duration::from_millis(500)) {
                            break;
                        }
                        thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        FeedServer { url, handle }
    }

    pub fn stop(self) {
        self.handle.join().unwrap();
    }
}
