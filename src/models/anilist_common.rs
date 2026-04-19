use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Title {
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
}

/// Which title variant the user's search input best matched.
///
/// Used to decide which variant is surfaced as the embed title vs the footer,
/// so the primary title mirrors what the user typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleVariant {
    English,
    Romaji,
    // Reserved for future native-input detection; today the fuzzy matcher
    // only scores against english/romaji, so this variant is never produced.
    #[allow(dead_code)]
    Native,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CoverImage {
    pub extra_large: Option<String>,
    pub large: Option<String>,
    pub medium: Option<String>,
    pub color: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ExternalLinks {
    pub url: String,
    #[serde(alias = "type")]
    pub url_type: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Tag {
    pub name: String,
}
