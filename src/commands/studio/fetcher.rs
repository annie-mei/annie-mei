use crate::{
    models::anilist_studio::Studio,
    utils::requests::anilist::{AniListRequestError, send_request},
};

use serde::Deserialize;
use serde_json::{Value, json};
use tracing::instrument;

const STUDIO_FIELDS: &str = r#"
    id
    name
    isAnimationStudio
    favourites
    siteUrl
    media(page: 1, perPage: 5, sort: POPULARITY_DESC, isMain: true) {
      nodes {
        id
        title {
          romaji
          english
        }
        siteUrl
        isAdult
      }
    }
"#;

const FETCH_STUDIO_BY_ID: &str = r#"
query ($id: Int) {
  Studio(id: $id) {
"#;

const FETCH_STUDIO_BY_SEARCH: &str = r#"
query ($search: String) {
  Studio(search: $search) {
"#;

const QUERY_END: &str = "  }\n}\n";

#[derive(Debug)]
pub enum StudioFetchError {
    Request(AniListRequestError),
    InvalidResponse(String),
    GraphQl(String),
}

impl std::fmt::Display for StudioFetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Request(error) => write!(f, "{error}"),
            Self::InvalidResponse(error) => {
                write!(f, "AniList returned an invalid studio response: {error}")
            }
            Self::GraphQl(error) => write!(f, "AniList returned a GraphQL error: {error}"),
        }
    }
}

impl std::error::Error for StudioFetchError {}

impl From<AniListRequestError> for StudioFetchError {
    fn from(error: AniListRequestError) -> Self {
        Self::Request(error)
    }
}

#[derive(Deserialize)]
struct StudioResponse {
    data: Option<StudioData>,
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Deserialize)]
struct StudioData {
    #[serde(rename = "Studio")]
    studio: Option<Studio>,
}

#[derive(Deserialize)]
struct GraphQlError {
    message: String,
}

#[instrument(name = "anilist.studio.fetch", fields(search_len = search_term.len()))]
pub async fn fetch_studio(search_term: &str) -> Result<Option<Studio>, StudioFetchError> {
    let request = build_request(search_term);
    let response = send_request(request).await?;
    parse_response(&response)
}

#[instrument(name = "anilist.studio.build_request", skip(search_term))]
fn build_request(search_term: &str) -> Value {
    match search_term.parse::<u32>() {
        Ok(id) => json!({
            "query": format!("{FETCH_STUDIO_BY_ID}{STUDIO_FIELDS}{QUERY_END}"),
            "variables": { "id": id },
        }),
        Err(_) => json!({
            "query": format!("{FETCH_STUDIO_BY_SEARCH}{STUDIO_FIELDS}{QUERY_END}"),
            "variables": { "search": search_term },
        }),
    }
}

#[instrument(name = "anilist.studio.parse_response", skip(response))]
fn parse_response(response: &str) -> Result<Option<Studio>, StudioFetchError> {
    let response: StudioResponse = serde_json::from_str(response)
        .map_err(|error| StudioFetchError::InvalidResponse(error.to_string()))?;

    if let Some(errors) = response.errors.filter(|errors| !errors.is_empty()) {
        return Err(StudioFetchError::GraphQl(
            errors
                .into_iter()
                .map(|error| error.message)
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }

    response.data.map(|data| data.studio).ok_or_else(|| {
        StudioFetchError::InvalidResponse("response did not contain data".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_input_builds_id_lookup() {
        let request = build_request("14");

        assert_eq!(request["variables"]["id"], 14);
        assert!(request["variables"].get("search").is_none());
        assert!(
            request["query"]
                .as_str()
                .unwrap()
                .contains("Studio(id: $id)")
        );
    }

    #[test]
    fn text_input_builds_search_lookup() {
        let request = build_request("Kyoto Animation");

        assert_eq!(request["variables"]["search"], "Kyoto Animation");
        assert!(request["variables"].get("id").is_none());
        assert!(
            request["query"]
                .as_str()
                .unwrap()
                .contains("Studio(search: $search)")
        );
    }

    #[test]
    fn null_studio_is_not_found() {
        let studio = parse_response(r#"{"data":{"Studio":null}}"#).unwrap();

        assert!(studio.is_none());
    }

    #[test]
    fn graphql_errors_are_not_treated_as_not_found() {
        let error =
            parse_response(r#"{"data":null,"errors":[{"message":"Unavailable"}]}"#).unwrap_err();

        assert!(matches!(error, StudioFetchError::GraphQl(_)));
    }
}
