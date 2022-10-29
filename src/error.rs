use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use deadpool::managed::PoolError;
use mongodb::bson::document::ValueAccessError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Return `400`
    #[error("parameters error")]
    ParamsError,

    #[error("an error occurred with the mongodb: {0}")]
    MongodbError(#[from] mongodb::error::Error),

    #[error("an error occurred when access the bson: {0}")]
    MongoValueError(#[from] ValueAccessError),

    #[error("an error occurred with the redis: {0}")]
    RedisError(#[from] deadpool_redis::redis::RedisError),

    #[error("an error occurred with the redis pool: {0}")]
    RedisPoolError(#[from] PoolError<deadpool_redis::redis::RedisError>),

    #[error("hash collision: {0}")]
    Overflow(String),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Self::ParamsError => {
                return (StatusCode::BAD_REQUEST, self.to_string()).into_response();
            }

            Self::MongodbError(_) | Self::MongoValueError(_) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response();
            }

            Self::RedisError(_) | Self::RedisPoolError(_) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response();
            }

            Self::Overflow(_) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response();
            }
        }
    }
}
