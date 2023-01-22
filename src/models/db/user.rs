use diesel::prelude::*;

#[derive(Queryable)]
pub struct User {
    pub discord_id: i64,
    pub anilist_id: i64,
    pub anilist_username: String,
}
