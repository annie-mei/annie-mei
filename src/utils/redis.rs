use redis::{Commands, Connection, RedisResult};
use std::env;
use tracing::info;

fn get_redis_client() -> RedisResult<Connection> {
    let user_name =
        env::var("REDIS_USERNAME").expect("Expected a redis username in the environment");
    let password =
        env::var("REDIS_PASSWORD").expect("Expected a redis password in the environment");
    let host = env::var("REDIS_HOST").expect("Expected a redis host in the environment");

    let redis_connection_string = format!("redis://{}:{}@{}", user_name, password, host);

    let client = redis::Client::open(redis_connection_string)?;
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
    redis_client_connection.set_ex(key, response, 18_000)?;
    Ok(())
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
