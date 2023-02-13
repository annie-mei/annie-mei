use std::collections::HashMap;

use serde_json::json;
use tokio::task;
use tracing::info;

use crate::{
    models::{
        db::user::User,
        user_media_list::{MediaListData, UserMediaList},
    },
    utils::{queries::FETCH_USER_MEDIA_LIST_DATA, requests::anilist::send_request},
};

pub async fn get_guild_data(
    guild_members: Vec<User>,
    media_id: u32,
    media_type: String,
) -> HashMap<i64, MediaListData> {
    let mut guild_scores: HashMap<i64, MediaListData> = HashMap::new();
    for user in guild_members {
        let body = json!({
            "query": FETCH_USER_MEDIA_LIST_DATA,
            "variables": {
                "userId": user.anilist_id,
                "type": media_type.to_uppercase(),
                "mediaId": media_id
            }
        });
        info!("Body: {:#?}", body);
        let user_media_list_response = task::spawn_blocking(move || send_request(body))
            .await
            .unwrap();
        let user_media_list_response: UserMediaList =
            serde_json::from_str(&user_media_list_response).unwrap();

        let media_list_data = user_media_list_response.data.unwrap();
        match media_list_data.media_list {
            None => continue,
            Some(data) => {
                guild_scores.insert(user.discord_id, data);
            }
        };
    }
    guild_scores
}
