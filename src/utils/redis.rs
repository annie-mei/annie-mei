use r2d2::{ManageConnection, NopErrorHandler, Pool};
use redis::{Commands, Connection, ConnectionLike, ErrorKind, RedisError, RedisResult};
use std::{env, sync::LazyLock, time::Duration};
use tracing::{info, instrument};

use crate::utils::{statics::REDIS_URL, tls::install_rustls_crypto_provider};

const CACHE_TTL_SECONDS: u64 = 18_000;
const REDIS_TIMEOUT: Duration = Duration::from_secs(2);
const REDIS_POOL_SIZE: u32 = 8;

#[derive(Clone)]
struct RedisConnectionManager {
    client: redis::Client,
}

impl ManageConnection for RedisConnectionManager {
    type Connection = Connection;
    type Error = RedisError;

    #[instrument(name = "redis.connect", skip_all)]
    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let connection = self.client.get_connection_with_timeout(REDIS_TIMEOUT)?;
        connection.set_read_timeout(Some(REDIS_TIMEOUT))?;
        connection.set_write_timeout(Some(REDIS_TIMEOUT))?;
        Ok(connection)
    }

    #[instrument(name = "redis.validate_connection", skip_all)]
    fn is_valid(&self, connection: &mut Self::Connection) -> Result<(), Self::Error> {
        redis::cmd("PING").query(connection)
    }

    #[instrument(name = "redis.connection_broken", skip_all)]
    fn has_broken(&self, connection: &mut Self::Connection) -> bool {
        !connection.is_open()
    }
}

struct RedisCache {
    pool: Pool<RedisConnectionManager>,
}

impl RedisCache {
    #[instrument(name = "redis.create_pool", skip(redis_url))]
    fn new(redis_url: &str) -> RedisResult<Self> {
        install_rustls_crypto_provider();

        let manager = RedisConnectionManager {
            client: redis::Client::open(redis_url)?,
        };
        let pool = Pool::builder()
            .max_size(REDIS_POOL_SIZE)
            .min_idle(Some(0))
            .connection_timeout(REDIS_TIMEOUT)
            .error_handler(Box::new(NopErrorHandler))
            .build_unchecked(manager);

        Ok(Self { pool })
    }

    #[instrument(name = "redis.check_cache", skip(self), fields(key = %key, key_len = key.len()))]
    fn get(&self, key: &str) -> RedisResult<String> {
        let mut connection = self.connection()?;
        connection.get(key)
    }

    #[instrument(name = "redis.cache_response", skip(self, response), fields(key = %key, key_len = key.len(), response_len = response.len()))]
    fn set(&self, key: &str, response: &str) -> RedisResult<()> {
        let mut connection = self.connection()?;
        connection.set_ex(key, response, CACHE_TTL_SECONDS)
    }

    #[instrument(name = "redis.get_pooled_connection", skip(self))]
    fn connection(&self) -> RedisResult<r2d2::PooledConnection<RedisConnectionManager>> {
        self.pool.get_timeout(REDIS_TIMEOUT).map_err(|error| {
            RedisError::from((
                ErrorKind::Io,
                "Timed out waiting for a Redis connection",
                error.to_string(),
            ))
        })
    }
}

static REDIS_CACHE: LazyLock<Result<RedisCache, String>> = LazyLock::new(|| {
    let redis_url = env::var(REDIS_URL)
        .map_err(|error| format!("Missing REDIS_URL environment variable: {error}"))?;
    RedisCache::new(&redis_url).map_err(|error| error.to_string())
});

#[instrument(name = "redis.get_cache", skip_all)]
fn redis_cache() -> RedisResult<&'static RedisCache> {
    REDIS_CACHE.as_ref().map_err(|error| {
        RedisError::from((
            ErrorKind::InvalidClientConfig,
            "Failed to configure Redis cache",
            error.clone(),
        ))
    })
}

#[instrument(name = "redis.check_cache", fields(key = %key, key_len = key.len()))]
pub fn check_cache(key: &str) -> RedisResult<String> {
    redis_cache()?.get(key)
}

#[instrument(name = "redis.cache_response", skip(response), fields(key = %key, key_len = key.len(), response_len = response.len()))]
fn cache_response(key: &str, response: &str) -> RedisResult<()> {
    redis_cache()?.set(key, response)
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
