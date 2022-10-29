mod config;
mod db;
mod error;
mod route;

use config::PORT;
use route::app;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], PORT));
    axum::Server::bind(&addr)
        .serve(app().await.into_make_service())
        .await
        .unwrap();
}
