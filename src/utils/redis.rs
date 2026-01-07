use redis::{Commands, Connection, RedisResult};
use std::env;
use tracing::info;

use crate::utils::statics::REDIS_URL;

fn get_redis_client() -> RedisResult<Connection> {
    let redis_url = env::var(REDIS_URL).expect("Expected REDIS_URL in the environment");

    let client = redis::Client::open(redis_url)?;
    let connection = client.get_connection()?;
    info!("Redis connection established");
    Ok(connection)
}

pub fn check_cache(key: &str) -> RedisResult<String> {
    let mut redis_client_connection = get_redis_client().unwrap();
    let cached_value: String = redis_client_connection.get(key)?;
    Ok(cached_value)
}

fn cache_response(key: &str, response: &str) -> RedisResult<()> {
    let mut redis_client_connection = get_redis_client().unwrap();
    // Expires cached value in 5 hours
    redis_client_connection.set_ex(key, response, 18_000)
}

pub fn try_to_cache_response(key: &str, response: &str) {
    match cache_response(key, response) {
        Ok(()) => {
            info!("Successfully cached {:#?}", key);
        }
        Err(e) => {
            info!("Failed to cache {:#?} with error {:#?}", key, e);
        }
    }
}
