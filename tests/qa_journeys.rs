//! Application journeys, not live Discord E2E. Run with `scripts/qa.sh`.
//! Only the HTTP boundaries are simulated; routing, SQLx, cache, parsing and
//! response serialization execute production code.

use std::{
    env,
    sync::Arc,
    time::{Duration, Instant},
};

use annie_mei::{
    commands::{context::CommandContext, dispatch_command, dispatch_component},
    utils::{
        database::{DatabasePoolKey, DbPool, run_migrations},
        requests::anilist::AniListClient,
    },
};
use axum::{
    Json, Router,
    extract::State,
    http::{Method, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use serenity::{
    all::{ApplicationId, Interaction, Message},
    cache::Cache,
    http::HttpBuilder,
    prelude::{RwLock, TypeMap},
};
use sqlx::postgres::PgPoolOptions;
use tokio::{net::TcpListener, sync::Mutex, task::JoinHandle, time::timeout};
use tracing::instrument;

const APPLICATION: u64 = 200000000000000002;
const USER: &str = "200000000000000005";
const OTHER_USER: &str = "200000000000000006";
const GUILD: &str = "200000000000000007";
const TOKEN: &str = "synthetic-qa-token";

#[derive(Clone, Copy)]
enum Upstream {
    Success,
    Unavailable,
    Timeout,
}

#[derive(Debug)]
struct Request {
    method: Method,
    path: String,
    body: Value,
    arrived_at: Instant,
}

#[derive(Clone)]
struct ServerState {
    requests: Arc<Mutex<Vec<Request>>>,
    upstream: Upstream,
}

// A recording server, not a Discord emulator: record exact wire requests,
// return valid response schemas, and reject every unspecified route.
#[instrument(skip_all)]
async fn record_request(
    State(state): State<ServerState>,
    method: Method,
    uri: Uri,
    Json(body): Json<Value>,
) -> Response {
    let path = uri.path();
    state.requests.lock().await.push(Request {
        method: method.clone(),
        path: path.replace(TOKEN, "<token>"),
        body,
        arrived_at: Instant::now(),
    });
    if method == Method::POST && path == "/graphql" {
        match state.upstream {
            Upstream::Success => {
                return Json(
                    serde_json::from_str::<Value>(include_str!("fixtures/anilist/qa-anime.json"))
                        .expect("valid fixture"),
                )
                .into_response();
            }
            Upstream::Unavailable => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
            Upstream::Timeout => {
                // Longer than both the injected HTTP timeout and journey deadline.
                tokio::time::sleep(Duration::from_secs(30)).await;
                return StatusCode::GATEWAY_TIMEOUT.into_response();
            }
        }
    }
    if method == Method::POST
        && path.starts_with("/api/v10/interactions/")
        && path.ends_with(&format!("/{TOKEN}/callback"))
    {
        return StatusCode::NO_CONTENT.into_response();
    }
    if method == Method::PATCH
        && path == format!("/api/v10/webhooks/{APPLICATION}/{TOKEN}/messages/@original")
    {
        return Json(message()).into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}

#[instrument(skip_all)]
fn message() -> Value {
    json!({
        "id": "200000000000000008", "channel_id": "200000000000000004",
        "author": {"id": APPLICATION.to_string(), "username": "qa-bot", "bot": true, "discriminator": "0000", "avatar": null},
        "content": "", "timestamp": "2026-01-01T00:00:00Z", "edited_timestamp": null,
        "tts": false, "mention_everyone": false, "mentions": [], "mention_roles": [],
        "attachments": [], "embeds": [], "pinned": false, "type": 0,
        "flags": 64, "components": []
    })
}

struct Journey {
    ctx: CommandContext,
    pool: DbPool,
    requests: Arc<Mutex<Vec<Request>>>,
    server: JoinHandle<()>,
    sequence: u64,
}

impl Journey {
    #[instrument(skip_all)]
    async fn new(upstream: Upstream) -> Self {
        // No DATABASE_URL fallback: accidental normal test invocation must not
        // touch a shared DB. The runner supplies a fresh loopback service.
        let database_url = env::var("QA_DATABASE_URL").expect("run scripts/qa.sh");
        let parsed = url::Url::parse(&database_url).expect("valid QA database URL");
        assert_eq!(parsed.host_str(), Some("127.0.0.1"));
        assert_eq!(parsed.username(), "annie_qa");
        let redis_url = env::var("REDIS_URL").expect("run scripts/qa.sh");
        assert_eq!(
            url::Url::parse(&redis_url)
                .expect("valid QA Redis URL")
                .host_str(),
            Some("127.0.0.1")
        );
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(Duration::from_secs(3))
            .connect(&database_url)
            .await
            .expect("disposable PostgreSQL available");
        run_migrations(&pool).await.expect("bot migrations apply");

        // Confirm the HTTP response fixture actually deserializes. The app's
        // existing adapters discard some edit errors, so {} is not sufficient.
        serde_json::from_value::<Message>(message()).expect("valid Discord message response");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new()
            .fallback(record_request)
            .with_state(ServerState {
                requests: requests.clone(),
                upstream,
            });
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test HTTP");
        let address = format!("http://{}", listener.local_addr().expect("local address"));
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("serve fixture requests");
        });
        let http = HttpBuilder::new("synthetic-bot-token")
            .proxy(&address)
            .application_id(ApplicationId::new(APPLICATION))
            .ratelimiter_disabled(true)
            .build();
        let mut data = TypeMap::new();
        data.insert::<DatabasePoolKey>(pool.clone());
        Self {
            ctx: CommandContext {
                http: Arc::new(http),
                cache: Arc::new(Cache::new()),
                data: Arc::new(RwLock::new(data)),
                anilist: AniListClient::new(
                    format!("{address}/graphql").parse().expect("local URL"),
                    Duration::from_millis(500),
                )
                .expect("AniList client"),
            },
            pool,
            requests,
            server,
            sequence: 100,
        }
    }

    #[instrument(skip_all)]
    async fn dispatch(&mut self, mut payload: Value, expected_upstream: bool) -> Value {
        self.sequence += 1;
        let id = 200000000000000000 + self.sequence;
        payload["id"] = json!(id.to_string());
        let component = payload["type"] == 3;
        let ephemeral = payload["data"]["name"] == "settings";
        let search = payload["data"]["options"][0]["value"].clone();
        let interaction: Interaction =
            serde_json::from_value(payload).expect("interaction fixture");
        let started = Instant::now();
        timeout(Duration::from_secs(5), async {
            match interaction {
                Interaction::Command(mut command) => {
                    dispatch_command(&self.ctx, &mut command).await
                }
                Interaction::Component(mut component) => {
                    dispatch_component(&self.ctx, &mut component).await
                }
                _ => panic!("expected command or component"),
            }
        })
        .await
        .expect("journey must complete within five seconds");

        let requests = std::mem::take(&mut *self.requests.lock().await);
        let expected_count = if expected_upstream { 3 } else { 2 };
        assert_eq!(
            requests.len(),
            expected_count,
            "unexpected HTTP sequence: {requests:?}"
        );
        assert_eq!(requests[0].method, Method::POST);
        assert_eq!(
            requests[0].path,
            format!("/api/v10/interactions/{id}/<token>/callback")
        );
        assert_eq!(requests[0].body["type"], if component { 6 } else { 5 });
        assert!(
            requests[0].arrived_at.duration_since(started) < Duration::from_secs(3),
            "Discord acknowledgement deadline exceeded"
        );
        let flags = requests[0].body["data"]["flags"].as_u64().unwrap_or(0);
        assert_eq!(flags, if ephemeral { 64 } else { 0 });
        if expected_upstream {
            assert_eq!(requests[1].body["variables"], json!({"search": search}));
            assert_eq!(requests[1].method, Method::POST);
            assert_eq!(
                requests[1].path, "/graphql",
                "must defer before upstream work"
            );
            assert!(
                requests[1].body["query"]
                    .as_str()
                    .expect("GraphQL query")
                    .contains("Page")
            );
        }
        let edit = requests.last().expect("response edit");
        assert_eq!(edit.method, Method::PATCH);
        assert_eq!(
            edit.path,
            format!("/api/v10/webhooks/{APPLICATION}/<token>/messages/@original")
        );
        edit.body.clone()
    }
}

impl Drop for Journey {
    #[instrument(skip_all)]
    fn drop(&mut self) {
        self.server.abort();
    }
}

#[instrument(skip_all)]
fn command(name: &str, user: &str, search: Option<&str>) -> Value {
    let mut payload: Value =
        serde_json::from_str(include_str!("fixtures/interactions/anime-invalid.json"))
            .expect("base interaction");
    payload["token"] = json!(TOKEN);
    payload["user"]["id"] = json!(user);
    payload["authorizing_integration_owners"]["1"] = json!(user);
    payload["data"]["name"] = json!(name);
    payload["data"]["options"] = search.map_or(json!([]), |search| {
        json!([{
            "name": "search", "type": 3, "value": search
        }])
    });
    payload
}

#[instrument(skip_all)]
fn component(user: &str, custom_id: &str, value: Option<&str>, permissions: Option<u64>) -> Value {
    let mut payload = command("settings", user, None);
    payload["type"] = json!(3);
    payload["data"] = match value {
        Some(value) => json!({"custom_id": custom_id, "component_type": 3, "values": [value]}),
        None => json!({"custom_id": custom_id, "component_type": 2}),
    };
    payload["message"] = message();
    if let Some(permissions) = permissions {
        payload["guild_id"] = json!(GUILD);
        payload["context"] = json!(0);
        payload["member"] = json!({
            "user": payload["user"].clone(), "roles": [], "joined_at": "2026-01-01T00:00:00Z",
            "deaf": false, "mute": false, "flags": 0, "permissions": permissions.to_string()
        });
        payload.as_object_mut().expect("object").remove("user");
    }
    payload
}

#[instrument(skip_all)]
fn content(response: &Value) -> &str {
    response["content"].as_str().expect("text response")
}

#[tokio::test]
#[ignore = "requires disposable services; run scripts/qa.sh"]
#[instrument]
async fn settings_persist_into_anime_response_and_real_cache() {
    let mut journey = Journey::new(Upstream::Success).await;
    let opened = journey
        .dispatch(command("settings", USER, None), false)
        .await;
    assert!(content(&opened).contains("User: not set"));
    let title_button = opened["components"][0]["components"][1]["custom_id"]
        .as_str()
        .expect("title button");
    assert_eq!(title_button, "settings:category:title_display");
    let category = journey
        .dispatch(component(USER, title_button, None, None), false)
        .await;
    let select = category["components"][1]["components"][0]["custom_id"]
        .as_str()
        .expect("user select");
    assert_eq!(select, "settings:set:user:title_display");
    let saved = journey
        .dispatch(component(USER, select, Some("english"), None), false)
        .await;
    assert!(content(&saved).contains("Saved `english`"));
    // Independent SQL assertion: do not derive expected state through production resolvers.
    let stored: String = sqlx::query_scalar("SELECT setting_value FROM annie_mei.user_settings WHERE discord_user_id = $1 AND setting_key = 'title_display'")
        .bind(USER).fetch_one(&journey.pool).await.expect("saved row");
    assert_eq!(stored, "english");
    let reopened = journey
        .dispatch(command("settings", USER, None), false)
        .await;
    assert!(content(&reopened).contains("Effective: `english` (English title) (user override)"));
    let category = journey
        .dispatch(component(USER, title_button, None, None), false)
        .await;
    let options = category["components"][1]["components"][0]["options"]
        .as_array()
        .expect("menu options");
    let selected: Vec<_> = options
        .iter()
        .filter(|option| option["default"] == true)
        .map(|option| option["value"].as_str().expect("value"))
        .collect();
    assert_eq!(selected, ["english"]);

    let anime = journey
        .dispatch(command("anime", USER, Some("Hoshi no Tabi")), true)
        .await;
    assert_eq!(anime["embeds"][0]["title"], "Journey Among Stars");
    assert_eq!(anime["embeds"][0]["url"], "https://anilist.co/anime/900001");
    let other = journey
        .dispatch(command("anime", OTHER_USER, Some("Hoshi no Tabi")), false)
        .await;
    assert_eq!(
        other["embeds"][0]["title"], "Hoshi No Tabi",
        "user preferences must not leak through cache"
    );

    let (cached, ttl): (String, i64) = tokio::task::spawn_blocking(|| {
        let client = redis::Client::open(env::var("REDIS_URL").expect("runner Redis URL"))
            .expect("Redis client");
        let mut connection = client.get_connection().expect("Redis connection");
        let value = redis::cmd("GET")
            .arg("Anime:Hoshi no Tabi")
            .query(&mut connection)
            .expect("cached response");
        let ttl = redis::cmd("TTL")
            .arg("Anime:Hoshi no Tabi")
            .query(&mut connection)
            .expect("cache TTL");
        (value, ttl)
    })
    .await
    .expect("cache assertions");
    assert_eq!(
        serde_json::from_str::<Value>(&cached).expect("cached JSON")["data"]["Page"]["media"][0]["id"],
        900001
    );
    assert!(
        (1..=18_000).contains(&ttl),
        "cache should expire, got {ttl}"
    );
}

#[tokio::test]
#[ignore = "requires disposable services; run scripts/qa.sh"]
#[instrument]
async fn guild_settings_require_manage_guild_and_preserve_stored_value() {
    let mut journey = Journey::new(Upstream::Success).await;
    let control = "settings:set:guild:title_display";
    let allowed = journey
        .dispatch(component(USER, control, Some("native"), Some(32)), false)
        .await;
    assert!(content(&allowed).contains("Saved `native`"));
    let denied = journey
        .dispatch(
            component(OTHER_USER, control, Some("english"), Some(0)),
            false,
        )
        .await;
    assert!(content(&denied).contains("You need the Manage Server permission"));
    let stored: String = sqlx::query_scalar("SELECT setting_value FROM annie_mei.guild_settings WHERE guild_id = $1 AND setting_key = 'title_display'")
        .bind(GUILD).fetch_one(&journey.pool).await.expect("guild row");
    assert_eq!(stored, "native");
    let personal_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM annie_mei.user_settings WHERE discord_user_id = $1",
    )
    .bind(OTHER_USER)
    .fetch_one(&journey.pool)
    .await
    .expect("personal rows");
    assert_eq!(
        personal_count, 0,
        "denied guild write must not become a personal write"
    );
}

#[tokio::test]
#[ignore = "requires disposable services; run scripts/qa.sh"]
#[instrument]
async fn anime_upstream_failure_finishes_deferred_response() {
    let mut journey = Journey::new(Upstream::Unavailable).await;
    let response = journey
        .dispatch(command("anime", USER, Some("qa unavailable anime")), true)
        .await;
    assert_eq!(content(&response), "I couldn't find that anime on AniList.");
    assert!(response.get("embeds").is_none());
}

#[tokio::test]
#[ignore = "requires disposable services; run scripts/qa.sh"]
#[instrument]
async fn anime_upstream_timeout_finishes_deferred_response() {
    let mut journey = Journey::new(Upstream::Timeout).await;
    let response = journey
        .dispatch(command("anime", USER, Some("qa timeout anime")), true)
        .await;
    // Preserve the current contract: timeouts and missing results share this copy.
    assert_eq!(content(&response), "I couldn't find that anime on AniList.");
    assert!(response.get("embeds").is_none());
}
