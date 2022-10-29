use deadpool::managed::{Pool, PoolConfig, Timeouts};
use deadpool_redis::{Config, Connection, Manager, Runtime};
use mongodb::{options::ClientOptions, Client};
use std::time::Duration;

pub async fn connect_db(dsc: &str) -> Client {
    let mut client_options = ClientOptions::parse(dsc).await.unwrap();
    client_options.app_name = Some("Shorturl".to_string());
    Client::with_options(client_options).unwrap()
}

pub async fn init_redis() -> Pool<Manager, Connection> {
    let mut pc = PoolConfig::default();
    let to = Timeouts {
        create: Some(Duration::from_millis(200)),
        wait: Some(Duration::from_millis(200)),
        recycle: Some(Duration::from_millis(200)),
    };
    pc.timeouts = to;
    let mut cfg = Config::from_url("redis://127.0.0.1:6379");
    cfg.pool = Some(pc);
    cfg.create_pool(Some(Runtime::Tokio1)).unwrap()
}
