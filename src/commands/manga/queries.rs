pub const FETCH_MANGA_BY_ID: &str = "
query ($id: Int) {
  Media (id: $id, type: ANIME) {
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
    tags {
      name
    }
  }
}
";

pub const FETCH_MANGA: &str = "
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
      tags {
        name
      }
    }
  }
}

";
