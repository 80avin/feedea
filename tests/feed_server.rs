#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

pub struct FeedServer {
    pub url: String,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl FeedServer {
    pub fn start(rss_body: String, connections: usize) -> FeedServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}/feed.xml");
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let handle = thread::spawn(move || {
            let mut served = 0;
            let started = std::time::Instant::now();
            while !thread_stop.load(Ordering::Relaxed) && served < connections {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        served += 1;
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
                        if started.elapsed() > std::time::Duration::from_secs(60) {
                            break;
                        }
                        thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        FeedServer { url, stop, handle: Some(handle) }
    }

    pub fn stop(mut self) {
        if let Some(handle) = self.handle.take() {
            self.stop.store(true, Ordering::Relaxed);
            handle.join().unwrap();
        }
    }
}

impl Drop for FeedServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
