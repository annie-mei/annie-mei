pub const FETCH_ANIME_BY_ID: &str = "
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

pub const FETCH_ANIME: &str = "
query ($page: Int, $perPage: Int, $search: String) {
  Page(page: $page, perPage: $perPage) {
    pageInfo {
      total
      currentPage
      lastPage
      hasNextPage
      perPage
    }
    media(search: $search) {
      type
      id
      idMal
      title {
        romaji
        english
        native
      }
      synonyms
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
}

";
