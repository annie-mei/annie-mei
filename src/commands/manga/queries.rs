pub const FETCH_MANGA_BY_ID: &str = "
query ($id: Int) {
  Media (id: $id, type: MANGA) {
    type
    id
    idMal
    title {
      romaji
      english
      native
    }
    synonyms
		startDate {
		  year
		  month
		  day
		}
    endDate {
      year
      month
      day
    }
    format
    status
    chapters
    volumes
    genres
    source
    coverImage {
      extraLarge
      large
      medium
      color
    }
    averageScore
    staff {
      edges {
        id
        role
      }
      nodes {
        id
        name {
          full
        }
        siteUrl
      }
    }
    siteUrl
    externalLinks {
      url
      type
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
      startDate {
        year
        month
        day
      }
      endDate {
        year
        month
        day
      }
      format
      status
      chapters
      volumes
      genres
      source
      coverImage {
        extraLarge
        large
        medium
        color
      }
      averageScore
      staff {
        edges {
          id
          role
        }
        nodes {
          id
          name {
            full
          }
          siteUrl
        }
      }
      siteUrl
      externalLinks {
        url
        type
      }
      description
      tags {
        name
      }
    }
  }
}

";
