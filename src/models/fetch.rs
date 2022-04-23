use super::anime::Anime;

#[derive(serde::Deserialize)]
pub struct FetchResponse {
    pub data: Option<FetchData>,
}

#[derive(serde::Deserialize)]
pub struct FetchData {
    #[serde(rename = "Media")]
    pub media: Option<Anime>,
}

// impl serde::Serialize for FetchRequest {
//   fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: serde::Serializer {
//     let mut request = serializer.serialize_struct("FetchRequest", 2)?;
//     request.serialize_field("query", /* Get the query for this variant */)?;
//     let mut variables = serializer.serialize_struct("variables", 1)?;
//     match self {
//       Self::Id(id) => { variables.serialize_field("id", id)?; },
//       Self::Search(search) => { variable.serialize_field("search", search)?; }
//     }
//     request.serialize_field("variables", variables.end()?)?;
//     request.end()
//   }
