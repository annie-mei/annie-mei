pub const FETCH_BY_ID_QUERY: &str = "
query ($id: Int) {
  Media (id: $id, type: ANIME) {
    id
    idMal
    title {
      romaji
      english
      native
    }
    season
    seasonYear
    format
    status
    episodes
    duration
    genres
    source
    coverImage {
      extraLarge
      large
      medium
      color
    }
    averageScore
    studios {
      edges {
        id
        isMain
      }
      nodes {
        id
        name
      }
    }
    siteUrl
    externalLinks {
      url
      type
    }
    trailer {
      id
      site
    }
    description
  }
}
";

pub const FETCH_BY_SEARCH_QUERY: &str = "
query ($search: String) {
  Media (search: $search, type: ANIME) {
    id
    idMal
    title {
      romaji
      english
      native
    }
    season
    seasonYear
    format
    status
    episodes
    duration
    genres
    source
    coverImage {
      extraLarge
      large
      medium
      color
    }
    averageScore
    studios {
      edges {
        id
        isMain
      }
      nodes {
        id
        name
      }
    }
    siteUrl
    externalLinks {
      url
      type
    }
    trailer {
      id
      site
    }
    description
  }
}
";
