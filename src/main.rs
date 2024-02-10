use std::{convert::Infallible, time::Duration};

use askama::Template;
use axum::{
    http::StatusCode,
    response::{sse::Event, IntoResponse, Response, Sse},
    routing::get,
    Extension, Router,
};
use axum::{routing::post, Json};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use tokio::{
    process::Command,
    sync::broadcast::{channel, Sender},
};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt as _};

pub type TodosStream = Sender<String>;

#[tokio::main]
async fn main() {
    let (tx, _rx) = channel::<String>(10);

    let app = Router::new()
        .route("/", get(root))
        .route("/styles.css", get(styles))
        .route("/stream", get(stream))
        .route("/todos/stream", get(handle_stream))
        .route("/users", post(create_user))
        .route("/cmd", post(handle_cmd))
        .layer(Extension(tx.clone()));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server listening on port 3000");

    tokio::spawn(async move {
        loop {
            println!("LOOPING");

            if tx.send("1".to_string()).is_err() {
                eprintln!("Error sending sse data across channel",);
            }
            sleep(Duration::from_millis(1000)).await;
        }
    });

    axum::serve(listener, app).await.unwrap();
}

pub async fn styles() -> impl IntoResponse {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/css")
        .body(include_str!("../templates/styles.css").to_owned())
        .unwrap()
}

#[derive(Template)]
#[template(path = "index.html")]
struct HelloTemplate;

async fn root() -> impl IntoResponse {
    HelloTemplate
}

#[derive(Template)]
#[template(path = "stream.html")]
struct StreamTemplate;

async fn stream() -> impl IntoResponse {
    StreamTemplate
}

async fn create_user(Json(payload): Json<CreateUser>) -> (StatusCode, Json<User>) {
    let user = User {
        id: 1337,
        username: payload.username,
    };
    (StatusCode::CREATED, Json(user))
}

#[derive(Deserialize)]
struct CreateUser {
    username: String,
}

#[derive(Serialize)]
struct User {
    id: u64,
    username: String,
}

pub async fn handle_stream(
    Extension(tx): Extension<TodosStream>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = tx.subscribe();

    let stream = BroadcastStream::new(rx);
    Sse::new(
        stream
            .map(|msg| {
                let msg = msg.unwrap();
                let json = format!("<div>{}</div>", msg);
                Event::default().data(json)
            })
            .map(Ok),
    )
    .keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(600))
            .text("keep-alive-text"),
    )
}

pub async fn handle_cmd(Extension(tx): Extension<TodosStream>) -> impl IntoResponse {
    let output = Command::new("echo")
        .arg("hello")
        .output()
        .await
        .expect("Should successfully run shell command");

    if tx.send(String::from_utf8(output.stdout).unwrap()).is_err() {
        eprintln!("Error sending cmd output across channel.");
    };

    (StatusCode::OK, "Successfully ran command.")
}
