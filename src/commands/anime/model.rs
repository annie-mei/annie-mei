use serde::{de::Error, Deserialize, Deserializer};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]

pub struct Anime {
    pub id: u32,
    pub id_mal: u32,
    pub title: Title,
    pub season: String,
    pub format: String,
    pub status: String,
    pub episodes: u32,
    pub duration: u32,
    pub genres: Vec<String>,
    pub source: String,
    pub cover_image: CoverImage,
    pub average_score: u32,
    pub studios: Studios,
    pub site_url: String,
    pub external_links: Vec<ExternalLinks>,
    #[serde(deserialize_with = "deserialize_trailer")]
    pub trailer: Option<Trailer>,
    pub description: String,
}

#[derive(Deserialize, Debug)]
pub struct Title {
    pub romaji: String,
    pub english: String,
    pub native: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]

pub struct CoverImage {
    pub extra_large: String,
    pub large: String,
    pub medium: String,
    pub color: String,
}

#[derive(Deserialize, Debug)]
pub struct Studios {
    pub edges: Vec<Edges>,
    pub nodes: Vec<Nodes>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Edges {
    pub id: u32,
    pub is_main: bool,
}

#[derive(Deserialize, Debug)]
pub struct Nodes {
    pub id: u32,
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct ExternalLinks {
    pub url: String,
    #[serde(alias = "type")]
    pub url_type: String,
}

#[derive(Deserialize, Debug)]
pub struct Trailer {
    pub id: String,
    pub site: String,
}

fn deserialize_trailer<'de, D>(d: D) -> Result<Option<Trailer>, D::Error>
where
    D: Deserializer<'de>,
{
    Deserialize::deserialize(d).map(|x: Option<_>| x.unwrap_or(None))
}
