use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]

pub struct Anime {
    pub id: u32,
    pub id_mal: u32,
    pub title: Title,
    pub season: String,
    pub season_year: String,
    pub format: String,
    pub status: String,
    pub episodes: Option<u32>,
    pub duration: Option<u32>,
    pub genres: Vec<String>,
    pub source: String,
    pub cover_image: CoverImage,
    pub average_score: Option<u32>,
    pub studios: Studios,
    pub site_url: String,
    pub external_links: Option<Vec<ExternalLinks>>,
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

impl Anime {
    pub fn transform_studios(&self) -> Vec<String> {
        let studios = &self.studios;
        // let main_studio_index = studios.edges.iter().position(|edge| edge.is_main);
        let mut main_studio_indices: Vec<usize> = Vec::new();

        for (index, edge) in studios.edges.iter().enumerate() {
            if edge.is_main {
                main_studio_indices.push(index);
            }
        }

        let mut main_studios: Vec<String> = Vec::new();

        for main_studio_index in main_studio_indices {
            main_studios.push(studios.nodes[main_studio_index].name.to_string())
        }

        main_studios
    }

    pub fn transform_trailer(&self) -> String {
        match &self.trailer {
            None => String::from("None"),
            Some(trailer) => format!("https://www.{}.com/watch?v={}", trailer.site, trailer.id),
        }
    }

    pub fn transform_color(&self) -> i32 {
        i32::from_str_radix(&self.cover_image.color.trim_start_matches("#"), 16).unwrap_or(0x0000ff)
    }

    pub fn transform_season(&self) -> String {
        format!("{} {}", &self.season, &self.season_year)
    }
}
