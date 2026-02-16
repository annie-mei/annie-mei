use diesel::prelude::*;
use serde_json::json;
use serenity::model::prelude::UserId;
use tracing::{info, instrument};

use crate::utils::{
    privacy::hash_user_id, queries::FETCH_ANILIST_USER, requests::anilist::send_request,
};

#[derive(Queryable)]
#[allow(dead_code)]
pub struct User {
    pub discord_id: i64,
    pub anilist_id: i64,
    pub anilist_username: String,
}

impl User {
    #[instrument(name = "db.user.get_by_discord_ids", skip(conn, user_discord_ids), fields(user_count = user_discord_ids.len()))]
    pub fn get_users_by_discord_id(
        user_discord_ids: Vec<UserId>,
        conn: &mut PgConnection,
    ) -> Option<Vec<User>> {
        use crate::schema::users::dsl::*;
        let user_discord_ids: Vec<i64> =
            user_discord_ids.iter().map(|id| id.get() as i64).collect();
        users
            .filter(discord_id.eq_any(user_discord_ids))
            .load::<User>(conn)
            .ok()
    }

    #[instrument(name = "db.user.create_or_update", skip(conn, anilist_username, discord_id), fields(discord_user_id = %hash_user_id(discord_id as u64), anilist_id = anilist_id, username_len = anilist_username.len()))]
    pub fn create_or_update_user(
        discord_id: i64,
        anilist_id: i64,
        anilist_username: String,
        conn: &mut PgConnection,
    ) -> User {
        use crate::schema::users;
        diesel::insert_into(users::table)
            .values((
                users::discord_id.eq(discord_id),
                users::anilist_id.eq(anilist_id),
                users::anilist_username.eq(anilist_username.to_owned()),
            ))
            .on_conflict(users::discord_id)
            .do_update()
            .set((
                users::anilist_id.eq(anilist_id),
                users::anilist_username.eq(anilist_username),
            ))
            .get_result(conn)
            .expect("Error saving user")
    }

    #[instrument(name = "http.anilist.lookup_user", fields(username_len = username.len()))]
    pub fn get_anilist_id_from_username(username: &str) -> Option<i64> {
        let body = json!({
            "query": FETCH_ANILIST_USER,
            "variables": {
                "username": username
            }
        });
        info!("Body: {:#?}", body);
        let result: String = send_request(body);
        info!("Result: {:#?}", result);
        let result: serde_json::Value = serde_json::from_str(&result).unwrap();

        result["data"]["User"]["id"].as_i64()
    }
}
