use diesel::prelude::*;
use serde_json::json;
use tracing::info;

use crate::utils::requests::anilist::send_request;

#[derive(Queryable)]
pub struct User {
    pub discord_id: i64,
    pub anilist_id: i64,
    pub anilist_username: String,
}

impl User {
    pub fn get_user_by_discord_id(user_discord_id: i64, conn: &mut PgConnection) -> Option<User> {
        use crate::schema::users::dsl::*;
        users
            .filter(discord_id.eq(user_discord_id))
            .first::<User>(conn)
            .ok()
    }

    pub fn create_user(
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
                users::anilist_username.eq(anilist_username),
            ))
            .get_result(conn)
            .expect("Error saving new user")
    }

    pub fn get_anilist_id_from_username(username: &str) -> i64 {
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

        result["data"]["User"]["id"].as_i64().unwrap()
    }
}
