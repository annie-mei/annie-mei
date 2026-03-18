use diesel::prelude::*;
use serde_json::json;
use serenity::model::prelude::UserId;
use tracing::{error, info, instrument};

use crate::utils::{
    privacy::hash_user_id, queries::FETCH_ANILIST_USER, requests::anilist::send_request,
};

#[derive(Debug)]
pub enum UserError {
    AniListRequest(String),
    AniListResponseParse(String),
    Database(diesel::result::Error),
}

impl std::fmt::Display for UserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserError::AniListRequest(error) => {
                write!(f, "Failed to fetch AniList user data: {error}")
            }
            UserError::AniListResponseParse(error) => {
                write!(f, "Failed to parse AniList user data response: {error}")
            }
            UserError::Database(error) => {
                write!(f, "Failed to persist user data: {error}")
            }
        }
    }
}

impl std::error::Error for UserError {}

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
    ) -> Result<User, UserError> {
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
            .map_err(UserError::Database)
    }

    #[instrument(name = "http.anilist.lookup_user", fields(username_len = username.len()))]
    pub fn get_anilist_id_from_username(username: &str) -> Result<Option<i64>, UserError> {
        let body = json!({
            "query": FETCH_ANILIST_USER,
            "variables": {
                "username": username
            }
        });
        info!("Body: {:#?}", body);
        let result: String = send_request(body).map_err(|error| {
            error!(error = %error, "AniList user lookup request failed");
            UserError::AniListRequest(error.to_string())
        })?;
        info!("Result: {:#?}", result);
        let result: serde_json::Value = serde_json::from_str(&result).map_err(|error| {
            error!(error = %error, "AniList user lookup response JSON parse failed");
            UserError::AniListResponseParse(error.to_string())
        })?;

        parse_anilist_user_id_response(&result)
    }
}

#[instrument(name = "http.anilist.parse_user_lookup_response", skip(result))]
fn parse_anilist_user_id_response(result: &serde_json::Value) -> Result<Option<i64>, UserError> {
    if result.get("errors").is_some() {
        let message = format!("AniList errors: {}", result["errors"]);
        error!(error = %message, "AniList GraphQL returned errors for user lookup");
        return Err(UserError::AniListResponseParse(message));
    }

    if result["data"]["User"].is_null() {
        info!("AniList user not found");
        return Ok(None);
    }

    if result["data"]["User"]["id"].is_null() {
        let message = "missing data.User.id".to_string();
        error!(error = %message, "AniList user lookup missing expected id field");
        return Err(UserError::AniListResponseParse(message));
    }

    let id = result["data"]["User"]["id"].as_i64().ok_or_else(|| {
        let message = "data.User.id is not an integer".to_string();
        error!(error = %message, "AniList user lookup id type mismatch");
        UserError::AniListResponseParse(message)
    })?;

    Ok(Some(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_user_lookup_returns_id_when_present() {
        let payload = json!({
            "data": {
                "User": {
                    "id": 12345
                }
            }
        });

        let result = parse_anilist_user_id_response(&payload);

        assert!(matches!(result, Ok(Some(12345))));
    }

    #[test]
    fn parse_user_lookup_returns_none_when_user_not_found() {
        let payload = json!({
            "data": {
                "User": null
            }
        });

        let result = parse_anilist_user_id_response(&payload);

        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn parse_user_lookup_errors_when_graphql_errors_present() {
        let payload = json!({
            "errors": [{
                "message": "User not found"
            }],
            "data": {
                "User": null
            }
        });

        let result = parse_anilist_user_id_response(&payload);

        assert!(matches!(
            result,
            Err(UserError::AniListResponseParse(message)) if message.contains("AniList errors")
        ));
    }

    #[test]
    fn parse_user_lookup_errors_when_id_missing() {
        let payload = json!({
            "data": {
                "User": {}
            }
        });

        let result = parse_anilist_user_id_response(&payload);

        assert!(matches!(
            result,
            Err(UserError::AniListResponseParse(message)) if message == "missing data.User.id"
        ));
    }

    #[test]
    fn parse_user_lookup_errors_when_id_not_integer() {
        let payload = json!({
            "data": {
                "User": {
                    "id": "abc"
                }
            }
        });

        let result = parse_anilist_user_id_response(&payload);

        assert!(matches!(
            result,
            Err(UserError::AniListResponseParse(message)) if message == "data.User.id is not an integer"
        ));
    }
}
