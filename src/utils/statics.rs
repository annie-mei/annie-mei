pub const NOT_FOUND_ANIME: &str = "No such anime";
pub const NOT_FOUND_MANGA: &str = "No such manga";
pub const NOT_FOUND_CHARACTER: &str = "No such character";
pub const NSFW_NOT_ALLOWED: &str =
    "This content is age-restricted and can only be viewed in NSFW channels";
pub const EMPTY_STR: &str = "-";
pub const ANILIST_STATUS_RELEASING: &str = "RELEASING";
#[cfg(test)]
pub const ANILIST_STATUS_FINISHED: &str = "FINISHED";

// Environment variables
pub const ENV: &str = "ENV";
pub const SENTRY_DSN: &str = "SENTRY_DSN";
pub const SENTRY_TRACES_SAMPLE_RATE: &str = "SENTRY_TRACES_SAMPLE_RATE";
pub const DISCORD_TOKEN: &str = "DISCORD_TOKEN";
pub const REDIS_URL: &str = "REDIS_URL";
pub const SPOTIFY_CLIENT_ID: &str = "SPOTIFY_CLIENT_ID";
pub const SPOTIFY_CLIENT_SECRET: &str = "SPOTIFY_CLIENT_SECRET";
pub const MAL_CLIENT_ID: &str = "MAL_CLIENT_ID";
pub const DATABASE_URL: &str = "DATABASE_URL";
pub const USERID_HASH_SALT: &str = "USERID_HASH_SALT";
pub const SERVER_PORT: &str = "SERVER_PORT";
pub const DEFAULT_SERVER_PORT: u16 = 8080;
pub const AUTH_SERVICE_BASE_URL: &str = "AUTH_SERVICE_BASE_URL";
pub const OAUTH_CONTEXT_SIGNING_SECRET: &str = "OAUTH_CONTEXT_SIGNING_SECRET";
pub const OAUTH_CONTEXT_TTL_SECONDS: &str = "OAUTH_CONTEXT_TTL_SECONDS";

// AI / LLM
pub const GEMINI_API_KEY: &str = "GEMINI_API_KEY";
pub const LLM_MODEL: &str = "LLM_MODEL";
pub const LLM_BASE_URL: &str = "LLM_BASE_URL";
