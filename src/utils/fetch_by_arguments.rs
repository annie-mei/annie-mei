use crate::utils::requests::anilist::{AniListRequestError, send_request};

use serde_json::json;
use tracing::{info, instrument};
use wana_kana::{ConvertJapanese, IsJapaneseStr};

/// Japanese particles reliable enough to split on in title searches.
/// Only `の` is used — it is almost always a true particle in compound
/// titles, whereas single-kana particles like は, と, で frequently
/// appear inside words (e.g. はたらく, とじまり, です).
const KANA_PARTICLES: &[&str] = &["の"];

/// Splits kana text at particle boundaries and transliterates each segment,
/// producing spaced romaji that AniList can match
/// (e.g. `きめつのやいば` → `kimetsu no yaiba`).
///
/// Only splits at particles that are both preceded and followed by at least one
/// other character, so leading/trailing particles are kept attached.
#[instrument(name = "kana.particle_spaced_romaji", fields(input_len = input.len()))]
fn kana_to_particle_spaced_romaji(input: &str) -> String {
    let mut split_positions: Vec<(usize, usize)> = Vec::new();

    for particle in KANA_PARTICLES {
        let particle_len = particle.len();
        let mut search_from = 0;
        while let Some(pos) = input[search_from..].find(particle) {
            let abs_pos = search_from + pos;
            if abs_pos > 0 && abs_pos + particle_len < input.len() {
                split_positions.push((abs_pos, abs_pos + particle_len));
            }
            search_from = abs_pos + particle_len;
        }
    }

    if split_positions.is_empty() {
        return input.to_romaji();
    }

    split_positions.sort_by_key(|&(start, _)| start);
    split_positions.dedup();

    let mut segments: Vec<&str> = Vec::new();
    let mut prev_end = 0;
    for &(start, end) in &split_positions {
        if start < prev_end {
            continue;
        }
        if start > prev_end {
            segments.push(&input[prev_end..start]);
        }
        segments.push(&input[start..end]);
        prev_end = end;
    }
    if prev_end < input.len() {
        segments.push(&input[prev_end..]);
    }

    segments
        .iter()
        .map(|s| s.to_romaji())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Checks whether an AniList Page response contains at least one media entry.
#[instrument(name = "anilist.has_media_results", skip(response))]
fn has_media_results(response: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(response)
        .ok()
        .and_then(|v| {
            v.get("data")
                .and_then(|d| d.get("Page"))
                .and_then(|p| p.get("media"))
                .and_then(|m| m.as_array())
                .map(|a| !a.is_empty())
        })
        .unwrap_or(false)
}

#[instrument(name = "anilist.fetch_by_id", skip(query), fields(id = id))]
pub async fn fetch_by_id(query: String, id: u32) -> Result<String, AniListRequestError> {
    let json = json!({"query": query, "variables": {"id":id}});
    let result = send_request(json).await?;

    info!("Fetched By ID: {:#?}", id);

    Ok(result)
}

#[instrument(name = "anilist.fetch_by_name", skip(query), fields(name_len = name.len()))]
pub async fn fetch_by_name(query: String, name: String) -> Result<String, AniListRequestError> {
    let searchable_name = if name.as_str().is_japanese() {
        // Strategy 1: try the original kana — AniList may match native titles directly
        let kana_json = json!({"query": &query, "variables": {"search": &name}});
        let kana_result = send_request(kana_json).await?;
        if has_media_results(&kana_result) {
            info!(strategy = "kana", "Fetched By Name: {:#?}", name);
            return Ok(kana_result);
        }

        // Strategy 2: particle-split romaji (e.g. きめつのやいば → kimetsu no yaiba)
        let spaced = kana_to_particle_spaced_romaji(&name);
        let compact = name.to_romaji();
        if spaced != compact {
            let spaced_json = json!({"query": &query, "variables": {"search": &spaced}});
            let spaced_result = send_request(spaced_json).await?;
            if has_media_results(&spaced_result) {
                info!(strategy = "spaced_romaji", "Fetched By Name: {:#?}", spaced);
                return Ok(spaced_result);
            }
        }

        // Fallback: compact romaji (original behaviour)
        compact
    } else {
        name.clone()
    };
    let json = json!({"query": query, "variables": {"search": &searchable_name}});
    let result = send_request(json).await?;

    info!("Fetched By Name: {:#?}", searchable_name);

    Ok(result)
}

#[instrument(name = "anilist.fetch_by_raw_name", skip(query), fields(name_len = name.len()))]
pub async fn fetch_by_raw_name(query: String, name: String) -> Result<String, AniListRequestError> {
    let json = json!({"query": query, "variables": {"search": name}});
    let result = send_request(json).await?;

    info!("Fetched By Raw Name");

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spaced_romaji_splits_at_no_particle() {
        let result = kana_to_particle_spaced_romaji("きめつのやいば");
        assert_eq!(result, "kimetsu no yaiba");
    }

    #[test]
    fn spaced_romaji_does_not_split_non_particle_kana() {
        // は inside a word is not treated as a particle
        let result = kana_to_particle_spaced_romaji("ぼくのなまえはたなかです");
        assert_eq!(result, "boku no namaehatanakadesu");
    }

    #[test]
    fn spaced_romaji_preserves_leading_particle() {
        // はたらくさいぼう — leading は is part of the word, not a separator
        let result = kana_to_particle_spaced_romaji("はたらくさいぼう");
        assert_eq!(result, "hatarakusaibou");
    }

    #[test]
    fn spaced_romaji_preserves_trailing_particle() {
        let result = kana_to_particle_spaced_romaji("たなかの");
        assert_eq!(result, "tanakano");
    }

    #[test]
    fn spaced_romaji_no_particles_returns_compact() {
        let result = kana_to_particle_spaced_romaji("なると");
        assert_eq!(result, "naruto");
    }

    #[test]
    fn spaced_romaji_single_particle_between_segments() {
        // すずめのとじまり → suzume no tojimari
        let result = kana_to_particle_spaced_romaji("すずめのとじまり");
        assert_eq!(result, "suzume no tojimari");
    }

    #[test]
    fn spaced_romaji_no_false_positive_particles() {
        // Single-kana particles inside words are not split
        let result = kana_to_particle_spaced_romaji("はたらくさいぼう");
        assert_eq!(result, "hatarakusaibou");
    }

    #[test]
    fn has_media_results_true_for_non_empty() {
        let json = r#"{"data":{"Page":{"media":[{"id":1}]}}}"#;
        assert!(has_media_results(json));
    }

    #[test]
    fn has_media_results_false_for_empty() {
        let json = r#"{"data":{"Page":{"media":[]}}}"#;
        assert!(!has_media_results(json));
    }

    #[test]
    fn has_media_results_false_for_missing_field() {
        let json = r#"{"data":{"Page":{}}}"#;
        assert!(!has_media_results(json));
    }

    #[test]
    fn has_media_results_false_for_invalid_json() {
        assert!(!has_media_results("not json"));
    }
}
