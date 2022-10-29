use crate::config::{
    APP_NAME, REDIS_ADDRESS, REDIS_CREATE_TIMEOUT, REDIS_RECYCLE_TIMEOUT, REDIS_WAIT_TIMEOUT,
};
use deadpool::managed::{Pool, PoolConfig, Timeouts};
use deadpool_redis::{Config, Connection, Manager, Runtime};
use mongodb::{options::ClientOptions, Client};
use std::time::Duration;

pub async fn connect_db(dsc: &str) -> Client {
    let mut client_options = ClientOptions::parse(dsc).await.unwrap();
    client_options.app_name = Some(APP_NAME.to_string());
    Client::with_options(client_options).unwrap()
}

pub async fn init_redis() -> Pool<Manager, Connection> {
    let mut pc = PoolConfig::default();
    let to = Timeouts {
        create: Some(Duration::from_millis(REDIS_CREATE_TIMEOUT)),
        wait: Some(Duration::from_millis(REDIS_WAIT_TIMEOUT)),
        recycle: Some(Duration::from_millis(REDIS_RECYCLE_TIMEOUT)),
    };
    pc.timeouts = to;
    let mut cfg = Config::from_url(REDIS_ADDRESS);
    cfg.pool = Some(pc);
    cfg.create_pool(Some(Runtime::Tokio1)).unwrap()
}
