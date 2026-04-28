use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct FetchResponse<T> {
    pub data: Option<FetchData<T>>,
}

#[derive(Deserialize, Debug)]
pub struct FetchData<T> {
    #[serde(rename = "Character")]
    pub character: Option<T>,
}
