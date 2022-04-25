use ngrammatic::{CorpusBuilder, Pad, SearchResult};
use tracing::info;

pub struct FuzzyResponse {
    pub index: usize,
    pub result: SearchResult,
}

pub fn fuzzy_match_title(
    pattern: String,
    string_list: Vec<String>,
    threshold: f32,
) -> Option<FuzzyResponse> {
    info!(
        "Matching {:#?} against {:#?} with a threshold of {:#?}",
        pattern, string_list, threshold
    );

    let mut corpus = CorpusBuilder::new()
        .arity(2)
        .pad_full(Pad::Auto)
        .case_insensitive()
        .finish();

    for title in string_list.iter() {
        corpus.add_text(title)
    }

    let results = corpus.search(&pattern, threshold);

    let response: Option<FuzzyResponse> = if results.first().is_some() {
        let top_match = results.first();
        let top_match_index = string_list
            .iter()
            .position(|title| *title == top_match.unwrap().text)
            .unwrap();
        info!("Top Match Index: {:#?}", top_match_index);
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
