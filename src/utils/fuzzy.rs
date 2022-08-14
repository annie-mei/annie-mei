use ngrammatic::{CorpusBuilder, Pad, SearchResult};
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct FuzzyResponse {
    pub result: SearchResult,
    pub index: usize,
}
impl Default for FuzzyResponse {
    fn default() -> Self {
        FuzzyResponse {
            index: usize::MAX,
            result: SearchResult {
                text: "".to_string(),
                similarity: 0.0,
            },
        }
    }
}

pub fn fuzzy_matcher(
    pattern: &str,
    string_list: Vec<String>,
    threshold: f32,
) -> Option<FuzzyResponse> {
    debug!(
        "Matching {:#?} against {:#?} with a threshold of {:#?}",
        pattern, string_list, threshold
    );

    let mut corpus = CorpusBuilder::new()
        .arity(2)
        .pad_full(Pad::Auto)
        .case_insensitive()
        .finish();

    for string in string_list.iter() {
        corpus.add_text(string)
    }

    let results = corpus.search(pattern, threshold);

    let response: Option<FuzzyResponse> = if results.first().is_some() {
        let top_match = results.first();
        info!("Top Match: {:#?}", top_match);
        let top_match_index = string_list
            .iter()
            .position(|string| *string.to_lowercase() == top_match.unwrap().text.to_lowercase())
            .unwrap();
        debug!("Top Match Index: {:#?}", top_match_index);
        info!("Top Match Similarity: {:#?}", top_match.unwrap().similarity);
        Some(FuzzyResponse {
            index: top_match_index,
            result: top_match.unwrap().clone(),
        })
    } else {
        None
    };

    response
}

pub fn fuzzy_matcher_synonyms(
    pattern: &str,
    synonyms_list: Vec<Vec<String>>,
) -> Option<FuzzyResponse> {
    debug!(
        "Matching {:#?} against Synonyms: {:#?}",
        pattern, synonyms_list
    );

    let results: Vec<Option<FuzzyResponse>> = synonyms_list
        .iter()
        .map(|synonyms| fuzzy_matcher(pattern, synonyms.to_vec(), 1.0))
        .collect();

    let match_index = results.iter().position(|result| result.is_some());

    info!("Synonyms Results:  {:#?}", results);

    match_index.map(|top_match_index| FuzzyResponse {
        index: top_match_index,
        result: results[top_match_index].as_ref().unwrap().result.clone(),
    })
}
