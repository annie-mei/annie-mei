use serde::Deserialize;
use std::fmt;

#[derive(Deserialize, Debug)]
pub struct UserMediaList {
    pub data: Option<MediaList>,
}

#[derive(Deserialize, Debug)]
pub struct MediaList {
    #[serde(rename = "MediaList")]
    pub media_list: Option<MediaListData>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MediaListData {
    pub status: Option<MediaListStatus>,
    pub score: Option<u32>,
    pub progress: Option<u32>,
    pub progress_volumes: Option<u32>,
}

#[derive(Deserialize, Debug)]
pub enum MediaListStatus {
    #[serde(rename = "CURRENT")]
    Current,
    #[serde(rename = "PLANNING")]
    Planning,
    #[serde(rename = "COMPLETED")]
    Completed,
    #[serde(rename = "DROPPED")]
    Dropped,
    #[serde(rename = "PAUSED")]
    Paused,
    #[serde(rename = "REPEATING")]
    Repeating,
}

impl fmt::Display for MediaListStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MediaListStatus::Current => write!(f, "Watching"),
            MediaListStatus::Planning => write!(f, "Planning"),
            MediaListStatus::Completed => write!(f, "Completed"),
            MediaListStatus::Dropped => write!(f, "Dropped"),
            MediaListStatus::Paused => write!(f, "Paused"),
            MediaListStatus::Repeating => write!(f, "Repeating"),
        }
    }
}

impl MediaListData {
    pub fn format_for_embed(&self, is_anime: bool) -> String {
        let mut embed = String::default();
        if let Some(status) = &self.status {
            let status = {
                if !is_anime && matches!(status, MediaListStatus::Current) {
                    "Reading".to_string()
                } else {
                    status.to_string()
                }
            };
            embed.push_str(&format!("**Status:** {}  ", status));
        }
        if let Some(score) = &self.score {
            embed.push_str(&format!("**Score:** {}  ", score));
        }

        // Skip other fields if status is completed
        if matches!(self.status, Some(MediaListStatus::Completed)) {
            return embed;
        } else {
            if let Some(progress) = &self.progress {
                embed.push_str(&format!("**Progress:** {progress}"));
            }
            if let Some(progress_volumes) = &self.progress_volumes {
                if progress_volumes != &0 {
                    embed.push_str(&format!("[{progress_volumes}]"));
                }
            }
        }

        embed
    }
}
