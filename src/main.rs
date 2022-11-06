use std::io::{BufReader, BufRead};
use std::net::SocketAddr;
use std::process::{Command, Stdio};

use axum;
use axum::extract::ws;
use axum::response::Response;
use axum::routing::{Router, get};


#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/download", get(handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], 4000));
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap()
}

async fn handler(ws: ws::WebSocketUpgrade) -> Response {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: ws::WebSocket) {
    while let Some(message) = socket.recv().await {
        let Ok(msg) = message else {
            return;
        };

        let Ok(url) = msg.to_text() else {
            return;
        };

        let mut yt_dlp = Command::new("yt-dlp")
            .arg(url)
            .stdout(Stdio::piped())
            .spawn()
            .expect("Can't create the yt-dlp process");

        let Some(stdout) = yt_dlp.stdout.take() else {
            return;
        };

        let mut buffer = BufReader::new(stdout);
        let mut line = String::with_capacity(52);
        loop {
            if buffer.read_line(&mut line).is_err() {
                break;
            }

            if line.len() == 0 {
                break;
            }

            match yt_dlp.try_wait() {
                Ok(Some(code)) => {
                    println!("exit code {code}");
                    break;
                },
                Ok(None) => {
                    let trimmed = line.trim().to_string();
                    socket.send(ws::Message::Text(trimmed)).await;
                    line.clear();
                }
                Err(code) => {
                    println!("error {code}");
                    break;
                }
            }
        }
    }
}
