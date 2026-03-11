/// A http file server to test http networking calls
use crate::repo_path;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::{Arc, Barrier};
use std::thread;
use tiny_http::{Request, Response, Server};

struct FileServerManager {
    port: u16,
    _server_thread: std::thread::JoinHandle<()>,
}

static FILE_SERVER_MANAGER: LazyLock<FileServerManager> = LazyLock::new(|| {
    // Barrier to synchronize server startup
    let barrier = Arc::new(Barrier::new(2));
    let barrier_clone = barrier.clone();
    let (startup_tx, startup_rx) = std::sync::mpsc::sync_channel(1);

    let server_thread = thread::spawn(move || {
        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind HTTP server port");
        let port = listener
            .local_addr()
            .expect("Failed to read HTTP server local address")
            .port();
        // Start the server
        let server = Server::from_listener(listener, None).expect("Failed to start HTTP server");
        startup_tx
            .send(port)
            .expect("Failed to communicate HTTP server port");

        // Signal that the server has started
        barrier_clone.wait();

        // Serve files
        for request in server.incoming_requests() {
            handle_request(request);
        }
    });

    let port = startup_rx
        .recv()
        .expect("Failed to receive HTTP server port");
    // Wait for the server to start
    barrier.wait();

    FileServerManager {
        port,
        _server_thread: server_thread,
    }
});

pub fn launch_file_server() {
    let _ = *FILE_SERVER_MANAGER;
}

pub fn file_server_port() -> u16 {
    FILE_SERVER_MANAGER.port
}

pub fn assert_file_server_ready(path: &str) {
    let mut stream = TcpStream::connect(("127.0.0.1", file_server_port()))
        .expect("Failed to connect to local HTTP server");
    let path = path.trim_start_matches('/');
    write!(
        stream,
        "GET /{path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )
    .expect("Failed to query local HTTP server");

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("Failed to read local HTTP server response");
    assert!(
        response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200"),
        "HTTP file server probe failed for /{path}: {response}"
    );
}

fn handle_request(request: Request) {
    // Build the file path
    let mut path = PathBuf::from(repo_path!());
    // Ensure the request URL doesn't contain '..' to prevent directory traversal
    let url_path = request.url().trim_start_matches('/').replace("..", "");
    path.push(url_path);

    // Serve the file if it exists
    if path.is_file() {
        if let Ok(mut file) = std::fs::File::open(&path) {
            let mut content = Vec::new();
            use std::io::Read;
            file.read_to_end(&mut content).unwrap();
            let response = Response::from_data(content);
            let _ = request.respond(response);
        } else {
            let response = Response::from_string("Internal Server Error").with_status_code(500);
            let _ = request.respond(response);
        }
    } else {
        let response = Response::from_string("Not Found").with_status_code(404);
        let _ = request.respond(response);
    }
}
