use axum::{
    extract::{Json, Query, State},
    routing::{get, post},
    Router,
};
use base64::{encode_config, CharacterSet, Config};
use deadpool::managed::Pool;
use deadpool_redis::{redis::cmd, Connection, Manager};
use fasthash::city;
use mongodb::{
    bson::{doc, Document},
    options::FindOneOptions,
    Client,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{AllowMethods, Any, CorsLayer};

use crate::{
    config::{DATABASE, MONGODB_ADDRESS},
    db::{connect_db, init_redis},
    error::Error,
};

const TABLES: [char; 64] = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l',
    'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4',
    '5', '6', '7', '8', '9', '+', '/',
];

#[derive(Clone)]
pub struct Srv {
    db: Client,
    rds: Pool<Manager, Connection>,
}

pub async fn app() -> Router<Srv> {
    let db = connect_db(MONGODB_ADDRESS).await;
    let rds = init_redis().await;
    let srv = Srv { db, rds };
    let cors = CorsLayer::new()
        .allow_methods(AllowMethods::any())
        .allow_origin(Any);
    Router::with_state(srv)
        .route("/", get(ok))
        .route("/short", post(short_url))
        .route("/query", get(query))
        .layer(cors)
}

async fn ok() -> &'static str {
    "OK"
}

fn collection_name(hash: &str) -> String {
    let firstchar = hash.chars().nth(0).unwrap();
    if firstchar.is_ascii_uppercase() {
        return String::from("upper");
    } else if firstchar.is_ascii_lowercase() {
        return String::from("lower");
    } else {
        return String::from("digit");
    }
}

#[derive(Deserialize)]
struct UrlBody {
    original_url: String,
}

/// Shortening url, when two URLs have same hash, increase the sequence,
///
/// but note that the maximum of sequence is 15.
async fn short_url(
    State(srv): State<Srv>,
    Json(body): Json<UrlBody>,
) -> Result<Json<Params>, Error> {
    if body.original_url.len() == 0 {
        return Err(Error::ParamsError);
    }
    let hash = city::hash32(&body.original_url);

    let conf = Config::new(CharacterSet::Standard, false);
    let short_url = encode_config(hash.to_be_bytes(), conf);
    let col = srv
        .db
        .database(DATABASE)
        .collection::<Document>(&collection_name(&short_url));
    let count = col
        .count_documents(doc! { "short_url": short_url.as_str() }, None)
        .await
        .map_err(|e| Error::MongodbError(e))? as u32;
    if count == 0 {
        col.insert_one(
            doc! { "short_url": short_url.as_str(), "original_url": body.original_url.as_str(), "seq": 0u32 },
            None,
        )
        .await
        .map_err(|e|Error::MongodbError(e))?;
        return Ok(Json(Params { short_url }));
    } else if count < 16 {
        let builder = FindOneOptions::builder();
        let opt = builder.projection(Some(doc! { "short_url": 1 })).build();
        let row = col.find_one(doc! { "short_url": short_url.as_str(), "original_url": body.original_url.as_str() }, Some(opt)).await.map_err(|e|Error::MongodbError(e))?;
        if let Some(row) = row {
            return Ok(Json(Params {
                short_url: row
                    .get_str("short_url")
                    .map_err(|e| Error::MongoValueError(e))?
                    .to_string(),
            }));
        }
        col.insert_one(
            doc! { "short_url": short_url.as_str(), "original_url": body.original_url.as_str(), "seq": count },
            None,
        )
        .await
        .map_err(|e|Error::MongodbError(e))?;
        return Ok(Json(Params { short_url }));
    } else {
        return Err(Error::Overflow(body.original_url));
    }
}

#[derive(Deserialize, Serialize)]
struct Params {
    short_url: String,
}

#[derive(Serialize)]
struct ShortUrlResp {
    original_url: String,
}

async fn query(
    State(srv): State<Srv>,
    Query(mut params): Query<Params>,
) -> Result<Json<ShortUrlResp>, Error> {
    let last = params.short_url.chars().last().ok_or(Error::ParamsError)?;
    let mut idx: usize = 0;
    for c in TABLES.iter() {
        if c == &last {
            break;
        }
        idx += 1;
    }
    let seq = (idx & 15) as u32;
    let hash = idx & 48;
    params
        .short_url
        .replace_range(params.short_url.len() - 1.., &TABLES[hash].to_string());
    let mut resp = ShortUrlResp {
        original_url: "".to_string(),
    };
    let key = format!("{}:{}", params.short_url.as_str(), seq);
    if let Some(url) = incache(&key, srv.rds.clone()).await? {
        resp.original_url.push_str(url.as_str());
        return Ok(Json(resp));
    }
    let col = srv
        .db
        .database(DATABASE)
        .collection::<Document>(&collection_name(&params.short_url));
    let builder = FindOneOptions::builder();
    let opt = builder.projection(Some(doc! { "original_url": 1 })).build();
    let row = col
        .find_one(
            doc! { "short_url": params.short_url.as_str(), "seq": seq },
            Some(opt),
        )
        .await
        .map_err(|e| Error::MongodbError(e))?;
    if let Some(row) = row {
        resp.original_url.push_str(
            row.get_str("original_url")
                .map_err(|e| Error::MongoValueError(e))?,
        );
        setcache(&key, &resp.original_url, srv.rds).await?;
    }
    return Ok(Json(resp));
}

async fn incache(key: &str, pool: Pool<Manager, Connection>) -> Result<Option<String>, Error> {
    let mut con = pool.get().await.map_err(|e| Error::RedisPoolError(e))?;
    Ok(cmd("GET").arg(key).query_async(&mut con).await.ok())
}

async fn setcache(key: &str, url: &str, pool: Pool<Manager, Connection>) -> Result<(), Error> {
    let mut con = pool.get().await.map_err(|e| Error::RedisPoolError(e))?;
    cmd("SETEX")
        .arg(key)
        .arg(10)
        .arg(url)
        .query_async::<_, bool>(&mut con)
        .await
        .map_err(|e| Error::RedisError(e))?;
    return Ok(());
}
