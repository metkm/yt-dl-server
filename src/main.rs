use std::io::{BufReader, BufRead, Error};
use std::net::SocketAddr;
use std::process::{Command, Stdio};
use std::env;

use axum;
use axum::extract::ws;
use axum::http::StatusCode;
use axum::response::{Response, IntoResponse};
use axum::routing::{Router, get, get_service};

use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    let videos_service = get_service(ServeDir::new("./videos")).handle_error(handle_error);

    let app = Router::new()
        .route("/download", get(handler))
        .nest("/videos", videos_service);

    let addr = SocketAddr::from(([0, 0, 0, 0], 4000));
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap()
}

async fn handle_error(_err: Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
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
            .arg("--no-simulate")
            .arg("--no-part")
            .arg("--q")
            .arg("--write-thumbnail")
            .args(["-S", "res,ext:mp4:m4a"])
            .args(["--recode", "mp4"])
            .args(["--paths", &format!("{}/videos", env::current_dir().unwrap().to_string_lossy())])
            .args(["--print", "after_move:[downloaded]:%(id)s.%(ext)s"])
            .args(["--print", "before_dl:[downloading]:%(id)s.%(ext)s"])
            .args(["--output", "%(id)s.%(ext)s"])
            .args(["--output", "thumbnail:%(id)s.%(ext)s"])
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

            let trimmed = line.trim().to_string();
            socket.send(ws::Message::Text(trimmed)).await.ok();
            line.clear();

            match yt_dlp.try_wait() {
                Ok(Some(code)) => {
                    println!("exit code {code}");
                    break;
                },
                Ok(None) => {}
                Err(code) => {
                    println!("error {code}");
                    break;
                }
            }
        }

        break;
    }
}
