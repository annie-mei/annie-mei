use std::sync::Arc;

use serenity::{
    cache::Cache,
    client::Context,
    http::{CacheHttp, Http},
    prelude::{RwLock, TypeMap},
};
use tracing::instrument;

use crate::utils::requests::anilist::AniListClient;

#[derive(Clone)]
pub struct CommandContext {
    pub http: Arc<Http>,
    pub cache: Arc<Cache>,
    pub data: Arc<RwLock<TypeMap>>,
    pub anilist: AniListClient,
}

impl CommandContext {
    #[instrument(name = "command.context.from_serenity", skip(ctx, anilist))]
    pub fn from_serenity(ctx: &Context, anilist: &AniListClient) -> Self {
        Self {
            http: Arc::clone(&ctx.http),
            cache: Arc::clone(&ctx.cache),
            data: Arc::clone(&ctx.data),
            anilist: anilist.clone(),
        }
    }
}

impl CacheHttp for CommandContext {
    #[instrument(name = "command.context.http", skip(self))]
    fn http(&self) -> &Http {
        &self.http
    }

    #[instrument(name = "command.context.cache", skip(self))]
    fn cache(&self) -> Option<&Arc<Cache>> {
        Some(&self.cache)
    }
}
