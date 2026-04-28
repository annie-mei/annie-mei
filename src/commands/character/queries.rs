pub const FETCH_CHARACTER_BY_ID: &str = "
query ($id: Int) {
  Character(id: $id) {
    id
    name {
      full
      native
      alternative
      alternativeSpoiler
      userPreferred
    }
    image {
      large
      medium
    }
    description(asHtml: true)
    gender
    dateOfBirth {
      year
      month
      day
    }
    age
    bloodType
    favourites
    siteUrl
    media(page: 1, perPage: 5, sort: POPULARITY_DESC) {
      nodes {
        id
        type
        title {
          romaji
          english
        }
        siteUrl
        isAdult
      }
    }
  }
}
";

pub const FETCH_CHARACTER: &str = "
query ($search: String) {
  Page(page: 1, perPage: 10) {
    characters(search: $search) {
      id
      name {
        full
        native
        alternative
        alternativeSpoiler
        userPreferred
      }
      image {
        large
        medium
      }
      description(asHtml: true)
      gender
      dateOfBirth {
        year
        month
        day
      }
      age
      bloodType
      favourites
      siteUrl
      media(page: 1, perPage: 5, sort: POPULARITY_DESC) {
        nodes {
          id
          type
          title {
            romaji
            english
          }
          siteUrl
          isAdult
        }
      }
    }
  }
}
";
