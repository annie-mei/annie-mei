const RECOMMENDATION_MEDIA_FIELDS: &str = "
    type
    id
    isAdult
    title {
      romaji
      english
      native
    }
    synonyms
    format
    status
    genres
    coverImage {
      extraLarge
      large
      medium
      color
    }
    averageScore
    siteUrl
    recommendations(sort: RATING_DESC, page: 1, perPage: 10) {
      nodes {
        rating
        mediaRecommendation {
          type
          id
          isAdult
          title {
            romaji
            english
            native
          }
          format
          status
          genres
          coverImage {
            extraLarge
            large
            medium
            color
          }
          averageScore
          siteUrl
        }
      }
    }
";

pub fn fetch_recommendations_by_id(media_type: &str) -> String {
    format!(
        "
query ($id: Int) {{
  Media(id: $id, type: {media_type}) {{
{RECOMMENDATION_MEDIA_FIELDS}
  }}
}}
"
    )
}

pub fn fetch_recommendations_by_search(media_type: &str) -> String {
    format!(
        "
query ($page: Int, $perPage: Int, $search: String) {{
  Page(page: $page, perPage: $perPage) {{
    media(search: $search, type: {media_type}) {{
{RECOMMENDATION_MEDIA_FIELDS}
    }}
  }}
}}
"
    )
}
