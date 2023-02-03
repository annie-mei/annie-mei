use diesel::prelude::*;
use serde_json::json;
use serenity::model::prelude::UserId;
use tracing::info;

use crate::utils::requests::anilist::send_request;

#[derive(Queryable)]
pub struct User {
    pub discord_id: i64,
    pub anilist_id: i64,
    pub anilist_username: String,
}

impl User {
    pub fn get_users_by_discord_id(
        user_discord_ids: Vec<UserId>,
        conn: &mut PgConnection,
    ) -> Option<Vec<User>> {
        use crate::schema::users::dsl::*;
        let user_discord_ids: Vec<i64> = user_discord_ids.iter().map(|id| id.0 as i64).collect();
        users
            .filter(discord_id.eq_any(user_discord_ids))
            .load::<User>(conn)
            .ok()
    }

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

    pub fn get_anilist_id_from_username(username: &str) -> Option<i64> {
        let body = json!({
            "query": "query ($username: String) { User (name: $username) { id } }",
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
