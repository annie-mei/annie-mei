use r2d2::{ManageConnection, NopErrorHandler, Pool};
use redis::{Cmd, Connection, ConnectionLike, ErrorKind, FromRedisValue, RedisError, RedisResult};
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

struct PooledRedisConnection {
    connection: Connection,
    poisoned: bool,
}

impl PooledRedisConnection {
    #[instrument(name = "redis.query", skip_all)]
    fn query<T: FromRedisValue>(&mut self, command: &Cmd) -> RedisResult<T> {
        let result = command.query(&mut self.connection);
        if result.as_ref().is_err_and(|error| error.is_io_error()) {
            self.poisoned = true;
        }
        result
    }
}

impl ManageConnection for RedisConnectionManager {
    type Connection = PooledRedisConnection;
    type Error = RedisError;

    #[instrument(name = "redis.connect", skip_all)]
    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let connection = self.client.get_connection_with_timeout(REDIS_TIMEOUT)?;
        connection.set_read_timeout(Some(REDIS_TIMEOUT))?;
        connection.set_write_timeout(Some(REDIS_TIMEOUT))?;
        Ok(PooledRedisConnection {
            connection,
            poisoned: false,
        })
    }

    #[instrument(name = "redis.validate_connection", skip_all)]
    fn is_valid(&self, connection: &mut Self::Connection) -> Result<(), Self::Error> {
        match connection.query::<String>(&redis::cmd("PING"))? {
            response if response == "PONG" => Ok(()),
            _ => Err(RedisError::from((
                ErrorKind::Client,
                "Redis connection validation returned an unexpected response",
            ))),
        }
    }

    #[instrument(name = "redis.connection_broken", skip_all)]
    fn has_broken(&self, connection: &mut Self::Connection) -> bool {
        connection.poisoned || !connection.connection.is_open()
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
        connection.query(redis::cmd("GET").arg(key))
    }

    #[instrument(name = "redis.cache_response", skip(self, response), fields(key = %key, key_len = key.len(), response_len = response.len()))]
    fn set(&self, key: &str, response: &str) -> RedisResult<()> {
        let mut connection = self.connection()?;
        connection.query(
            redis::cmd("SETEX")
                .arg(key)
                .arg(CACHE_TTL_SECONDS)
                .arg(response),
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::HashMap,
        io::{BufRead, BufReader, Read, Write},
        net::{TcpListener, TcpStream},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        thread,
        time::Instant,
    };

    struct MockRedis {
        address: String,
        accepted_connections: Arc<AtomicUsize>,
        values: Arc<Mutex<HashMap<String, String>>>,
        ttl: Arc<Mutex<Option<u64>>>,
        delay_next_get: Arc<AtomicBool>,
        stopping: Arc<AtomicBool>,
        server: Option<thread::JoinHandle<()>>,
    }

    impl MockRedis {
        fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock Redis");
            listener
                .set_nonblocking(true)
                .expect("make mock Redis nonblocking");
            let address = listener
                .local_addr()
                .expect("read mock Redis address")
                .to_string();
            let accepted_connections = Arc::new(AtomicUsize::new(0));
            let ttl = Arc::new(Mutex::new(None));
            let delay_next_get = Arc::new(AtomicBool::new(false));
            let stopping = Arc::new(AtomicBool::new(false));
            let values = Arc::new(Mutex::new(HashMap::new()));

            let server_connections = Arc::clone(&accepted_connections);
            let server_ttl = Arc::clone(&ttl);
            let server_delay_next_get = Arc::clone(&delay_next_get);
            let server_stopping = Arc::clone(&stopping);
            let server_values = Arc::clone(&values);
            let server = thread::spawn(move || {
                while !server_stopping.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            server_connections.fetch_add(1, Ordering::Relaxed);
                            let values = Arc::clone(&server_values);
                            let ttl = Arc::clone(&server_ttl);
                            let delay_next_get = Arc::clone(&server_delay_next_get);
                            thread::spawn(move || {
                                serve_connection(stream, &values, &ttl, &delay_next_get);
                            });
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("mock Redis accept failed: {error}"),
                    }
                }
            });

            Self {
                address,
                accepted_connections,
                values,
                ttl,
                delay_next_get,
                stopping,
                server: Some(server),
            }
        }

        fn url(&self) -> String {
            format!("redis://{}/", self.address)
        }

        fn insert(&self, key: &str, value: &str) {
            self.values
                .lock()
                .expect("lock mock values")
                .insert(key.to_string(), value.to_string());
        }

        fn delay_next_get(&self) {
            self.delay_next_get.store(true, Ordering::Relaxed);
        }
    }

    impl Drop for MockRedis {
        fn drop(&mut self) {
            self.stopping.store(true, Ordering::Relaxed);
            let _ = TcpStream::connect(&self.address);
            if let Some(server) = self.server.take() {
                server.join().expect("stop mock Redis");
            }
        }
    }

    fn serve_connection(
        stream: TcpStream,
        values: &Mutex<HashMap<String, String>>,
        ttl: &Mutex<Option<u64>>,
        delay_next_get: &AtomicBool,
    ) {
        let mut writer = stream.try_clone().expect("clone mock Redis stream");
        let mut reader = BufReader::new(stream);

        while let Some(command) = read_command(&mut reader) {
            match command[0].to_ascii_uppercase().as_str() {
                "PING" => writer.write_all(b"+PONG\r\n").expect("reply to PING"),
                "CLIENT" => writer.write_all(b"+OK\r\n").expect("reply to CLIENT"),
                "GET" => {
                    if delay_next_get.swap(false, Ordering::Relaxed) {
                        thread::sleep(Duration::from_millis(2_100));
                    }
                    match values.lock().expect("lock mock values").get(&command[1]) {
                        Some(value) => writer
                            .write_all(format!("${}\r\n{value}\r\n", value.len()).as_bytes())
                            .expect("reply to GET"),
                        None => writer.write_all(b"$-1\r\n").expect("reply to GET miss"),
                    }
                }
                "SETEX" => {
                    values
                        .lock()
                        .expect("lock mock values")
                        .insert(command[1].clone(), command[3].clone());
                    *ttl.lock().expect("lock mock TTL") =
                        Some(command[2].parse().expect("parse SETEX TTL"));
                    writer.write_all(b"+OK\r\n").expect("reply to SETEX");
                }
                command => panic!("unexpected Redis command: {command}"),
            }
        }
    }

    fn read_command(reader: &mut BufReader<TcpStream>) -> Option<Vec<String>> {
        let mut line = String::default();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let argument_count: usize = line.strip_prefix('*')?.trim().parse().ok()?;
        let mut command = Vec::with_capacity(argument_count);

        for _ in 0..argument_count {
            line.clear();
            reader.read_line(&mut line).ok()?;
            let length: usize = line.strip_prefix('$')?.trim().parse().ok()?;
            let mut argument = vec![0; length];
            reader.read_exact(&mut argument).ok()?;
            let mut crlf = [0; 2];
            reader.read_exact(&mut crlf).ok()?;
            command.push(String::from_utf8(argument).ok()?);
        }

        Some(command)
    }

    #[test]
    fn cache_hit_miss_write_ttl_and_connection_reuse() {
        let redis = MockRedis::start();
        let cache = RedisCache::new(&redis.url()).expect("create Redis cache");

        assert!(cache.get("missing").is_err());
        cache.set("key", "value").expect("write cached value");
        assert_eq!(cache.get("key").expect("read cached value"), "value");
        assert_eq!(*redis.ttl.lock().expect("lock mock TTL"), Some(18_000));
        assert_eq!(redis.accepted_connections.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn unavailable_redis_is_bounded() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("reserve unavailable port");
        let url = format!("redis://{}/", listener.local_addr().expect("read address"));
        drop(listener);
        let cache = RedisCache::new(&url).expect("create Redis cache");

        let started = Instant::now();
        assert!(cache.get("key").is_err());
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn command_wait_is_bounded() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stalled Redis");
        let url = format!("redis://{}/", listener.local_addr().expect("read address"));
        let server = thread::spawn(move || {
            let (_stream, _) = listener.accept().expect("accept stalled connection");
            thread::sleep(Duration::from_secs(3));
        });
        let cache = RedisCache::new(&url).expect("create Redis cache");

        let started = Instant::now();
        assert!(cache.get("key").is_err());
        assert!(started.elapsed() < Duration::from_secs(3));
        server.join().expect("stop stalled Redis");
    }

    #[test]
    fn timed_out_connection_is_not_reused() {
        let redis = MockRedis::start();
        redis.insert("slow", "stale");
        redis.insert("fresh", "fresh");
        redis.delay_next_get();
        let cache = RedisCache::new(&redis.url()).expect("create Redis cache");

        assert!(cache.get("slow").is_err());
        assert_eq!(cache.get("fresh").expect("read fresh value"), "fresh");
        assert_eq!(redis.accepted_connections.load(Ordering::Relaxed), 2);
        thread::sleep(Duration::from_millis(150));
    }
}
