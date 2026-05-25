use serde::Deserialize;
use std::fmt;
use tracing::instrument;

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
    #[instrument(skip(self))]
    pub fn format_for_embed(&self, is_anime: bool) -> String {
        let mut parts = Vec::new();

        if let Some(status) = &self.status {
            parts.push(status_phrase(status, is_anime).to_string());
        }

        if !matches!(self.status, Some(MediaListStatus::Completed))
            && !matches!(self.status, Some(MediaListStatus::Planning))
            && let Some(progress) = self.progress
        {
            parts.push(progress_phrase(
                progress,
                self.progress_volumes,
                self.status.as_ref(),
                is_anime,
            ));
        }

        if let Some(score) = self.score
            && score > 0
        {
            parts.push(format!("rated {score}/100"));
        }

        parts.join(", ")
    }
}

#[instrument]
fn status_phrase(status: &MediaListStatus, is_anime: bool) -> &'static str {
    match status {
        MediaListStatus::Current if is_anime => "is watching",
        MediaListStatus::Current => "is reading",
        MediaListStatus::Planning if is_anime => "plans to watch it",
        MediaListStatus::Planning => "plans to read it",
        MediaListStatus::Completed => "finished it",
        MediaListStatus::Dropped => "dropped it",
        MediaListStatus::Paused => "paused it",
        MediaListStatus::Repeating if is_anime => "is rewatching",
        MediaListStatus::Repeating => "is rereading",
    }
}

#[instrument]
fn progress_phrase(
    progress: u32,
    progress_volumes: Option<u32>,
    status: Option<&MediaListStatus>,
    is_anime: bool,
) -> String {
    let progress_text = if is_anime {
        format!("{progress} {}", pluralize(progress, "ep", "eps"))
    } else {
        let mut text = format!("{progress} {}", pluralize(progress, "chapter", "chapters"));
        if let Some(volumes) = progress_volumes
            && volumes > 0
        {
            text.push_str(&format!(
                " / {volumes} {}",
                pluralize(volumes, "vol", "vols")
            ));
        }
        text
    };

    match status {
        Some(MediaListStatus::Dropped) => format!("after {progress_text}"),
        Some(MediaListStatus::Paused) => format!("at {progress_text}"),
        _ => format!("{progress_text} in"),
    }
}

#[instrument]
fn pluralize(count: u32, singular: &'static str, plural: &'static str) -> &'static str {
    if count == 1 { singular } else { plural }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_anime_formats_as_sentence() {
        let data = MediaListData {
            status: Some(MediaListStatus::Current),
            score: Some(91),
            progress: Some(5),
            progress_volumes: None,
        };

        assert_eq!(
            data.format_for_embed(true),
            "is watching, 5 eps in, rated 91/100"
        );
    }

    #[test]
    fn current_manga_formats_chapters_and_volumes() {
        let data = MediaListData {
            status: Some(MediaListStatus::Current),
            score: Some(88),
            progress: Some(12),
            progress_volumes: Some(2),
        };

        assert_eq!(
            data.format_for_embed(false),
            "is reading, 12 chapters / 2 vols in, rated 88/100"
        );
    }

    #[test]
    fn completed_media_skips_progress() {
        let data = MediaListData {
            status: Some(MediaListStatus::Completed),
            score: Some(100),
            progress: Some(24),
            progress_volumes: None,
        };

        assert_eq!(data.format_for_embed(true), "finished it, rated 100/100");
    }

    #[test]
    fn missing_optional_values_are_omitted() {
        let data = MediaListData {
            status: Some(MediaListStatus::Planning),
            score: None,
            progress: Some(0),
            progress_volumes: Some(0),
        };

        assert_eq!(data.format_for_embed(false), "plans to read it");
    }

    #[test]
    fn zero_score_is_omitted_as_unscored() {
        let data = MediaListData {
            status: Some(MediaListStatus::Current),
            score: Some(0),
            progress: Some(3),
            progress_volumes: None,
        };

        assert_eq!(data.format_for_embed(true), "is watching, 3 eps in");
    }
}
