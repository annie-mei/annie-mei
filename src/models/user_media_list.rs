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
    pub media: Option<MediaListMedia>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MediaListMedia {
    pub episodes: Option<u32>,
    pub volumes: Option<u32>,
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
    pub fn format_for_embed(&self) -> String {
        let mut embed = String::new();
        if let Some(status) = &self.status {
            embed.push_str(&format!("**Status:** {}  ", status));
        }
        if let Some(score) = &self.score {
            embed.push_str(&format!("**Score:** {}  ", score));
        }

        // Skip other fields if status is completed
        if let Some(MediaListStatus::Completed) = &self.status {
            return embed
        } else {
            if let Some(progress) = &self.progress {
                embed.push_str(&format!("**Progress:** {progress}"));
            }
            if let Some(progress_volumes) = &self.progress_volumes {
                embed.push_str(&format!("**Progress:** {progress_volumes}"));
            }
            if let Some(media) = &self.media {
                if let Some(episodes) = &media.episodes {
                    embed.push_str(&format!("/{episodes}"));
                }
                if let Some(volumes) = &media.volumes {
                    embed.push_str(&format!("/{volumes}"));
                }
            }
        }

        embed
    }
}
