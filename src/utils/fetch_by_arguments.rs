use crate::utils::requests::anilist::{AniListRequestError, send_request};

use serde_json::json;
use tracing::{info, instrument, warn};
use wana_kana::{ConvertJapanese, IsJapaneseStr};

/// Japanese particles reliable enough to split on in title searches.
/// Only the hiragana/katakana forms of `no` are used — they are almost always
/// true particles in compound titles, whereas single-kana particles like は,
/// と, で frequently appear inside words (e.g. はたらく, とじまり, です).
const KANA_PARTICLES: &[&str] = &["の", "ノ"];

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
///
/// Returns `None` when the response carries a GraphQL-level error (so the
/// caller can short-circuit instead of retrying with a different search term).
/// Returns `Some(true)` when media results exist, `Some(false)` when the
/// response is valid but empty.
#[instrument(name = "anilist.has_media_results", skip(response))]
fn has_media_results(response: &str) -> Option<bool> {
    let value: serde_json::Value = match serde_json::from_str(response) {
        Ok(v) => v,
        Err(_) => return None,
    };

    if value
        .get("errors")
        .and_then(|e| e.as_array())
        .is_some_and(|a| !a.is_empty())
    {
        warn!("AniList response contains GraphQL errors");
        return None;
    }

    Some(
        value
            .get("data")
            .and_then(|d| d.get("Page"))
            .and_then(|p| p.get("media"))
            .and_then(|m| m.as_array())
            .is_some_and(|a| !a.is_empty()),
    )
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
    if name.as_str().is_japanese() {
        // Strategy 1: try the original Japanese input, but only keep it as a fallback.
        // Short kana abbreviations can return weak partial native-title matches, while
        // AniList often has a stronger romaji synonym/search match (e.g. このすば → konosuba).
        let kana_json = json!({"query": &query, "variables": {"search": &name}});
        let kana_result = send_request(kana_json).await?;
        let kana_fallback = match has_media_results(&kana_result) {
            Some(true) => Some(kana_result),
            None => {
                // GraphQL error — skip further strategies, return as-is
                return Ok(kana_result);
            }
            Some(false) => None,
        };

        // Strategy 2: particle-split romaji (e.g. きめつのやいば → kimetsu no yaiba)
        let spaced = kana_to_particle_spaced_romaji(&name);
        let compact = name.to_romaji();
        if spaced != compact {
            let spaced_json = json!({"query": &query, "variables": {"search": &spaced}});
            let spaced_result = send_request(spaced_json).await?;
            match has_media_results(&spaced_result) {
                Some(true) => {
                    info!(strategy = "spaced_romaji", "Fetched By Name: {:#?}", spaced);
                    return Ok(spaced_result);
                }
                None => return Ok(spaced_result),
                Some(false) => {}
            }
        }

        // Strategy 3: compact romaji (original behaviour). Prefer this over raw
        // Japanese fallback when it has candidates, because those candidates tend
        // to align better with downstream romaji/English fuzzy matching.
        let compact_json = json!({"query": query, "variables": {"search": &compact}});
        let compact_result = send_request(compact_json).await?;
        match has_media_results(&compact_result) {
            Some(true) => {
                info!(
                    strategy = "compact_romaji",
                    "Fetched By Name: {:#?}", compact
                );
                Ok(compact_result)
            }
            None => Ok(compact_result),
            Some(false) => {
                if let Some(kana_result) = kana_fallback {
                    info!(
                        strategy = "japanese_fallback",
                        "Fetched By Name: {:#?}", name
                    );
                    Ok(kana_result)
                } else {
                    Ok(compact_result)
                }
            }
        }
    } else {
        let searchable_name = name.clone();
        let json = json!({"query": query, "variables": {"search": &searchable_name}});
        let result = send_request(json).await?;

        info!("Fetched By Name: {:#?}", searchable_name);

        Ok(result)
    }
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
    fn spaced_romaji_splits_at_katakana_no_particle() {
        let result = kana_to_particle_spaced_romaji("キメツノヤイバ");
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
    fn spaced_romaji_medial_non_no_particle_not_split() {
        // おとうと contains と in a medial position but it is not a split particle
        let result = kana_to_particle_spaced_romaji("おとうと");
        assert_eq!(result, "otouto");
    }

    #[test]
    fn has_media_results_some_true_for_non_empty() {
        let json = r#"{"data":{"Page":{"media":[{"id":1}]}}}"#;
        assert_eq!(has_media_results(json), Some(true));
    }

    #[test]
    fn has_media_results_some_false_for_empty() {
        let json = r#"{"data":{"Page":{"media":[]}}}"#;
        assert_eq!(has_media_results(json), Some(false));
    }

    #[test]
    fn has_media_results_some_false_for_missing_field() {
        let json = r#"{"data":{"Page":{}}}"#;
        assert_eq!(has_media_results(json), Some(false));
    }

    #[test]
    fn has_media_results_none_for_invalid_json() {
        assert_eq!(has_media_results("not json"), None);
    }

    #[test]
    fn has_media_results_none_for_graphql_errors() {
        let json = r#"{"data":null,"errors":[{"message":"query error"}]}"#;
        assert_eq!(has_media_results(json), None);
    }
}
