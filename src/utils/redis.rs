use redis::{Commands, Connection, RedisResult};
use std::env;
use tracing::{info, instrument};

use crate::utils::statics::REDIS_URL;

#[instrument(name = "redis.get_connection", skip_all)]
fn get_redis_client() -> RedisResult<Connection> {
    let redis_url = env::var(REDIS_URL).expect("Expected REDIS_URL in the environment");

    let client = redis::Client::open(redis_url)?;
    let connection = client.get_connection()?;
    info!("Redis connection established");
    Ok(connection)
}

#[instrument(name = "redis.check_cache", fields(key = %key, key_len = key.len()))]
pub fn check_cache(key: &str) -> RedisResult<String> {
    let mut redis_client_connection = get_redis_client().unwrap();
    let cached_value: String = redis_client_connection.get(key)?;
    Ok(cached_value)
}

#[instrument(name = "redis.cache_response", skip(response), fields(key = %key, key_len = key.len(), response_len = response.len()))]
fn cache_response(key: &str, response: &str) -> RedisResult<()> {
    let mut redis_client_connection = get_redis_client().unwrap();
    // Expires cached value in 5 hours
    redis_client_connection.set_ex(key, response, 18_000)
}

#[instrument(name = "redis.try_cache_response", skip(response), fields(key = %key, key_len = key.len(), response_len = response.len()))]
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
