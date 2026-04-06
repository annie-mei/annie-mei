use diesel::prelude::*;
use serenity::model::prelude::UserId;
use tracing::instrument;

#[derive(Queryable)]
#[allow(dead_code)]
pub struct User {
    pub discord_id: i64,
    pub anilist_id: i64,
    pub anilist_username: String,
}

impl User {
    #[instrument(name = "db.user.get_by_discord_id", skip(conn), fields(discord_user_id = %hash_user_id(user_discord_id as u64)))]
    pub fn get_user_by_discord_id(user_discord_id: i64, conn: &mut PgConnection) -> Option<User> {
        use crate::schema::users::dsl::*;

        users
            .filter(discord_id.eq(user_discord_id))
            .first::<User>(conn)
            .optional()
            .ok()
            .flatten()
    }

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
}
