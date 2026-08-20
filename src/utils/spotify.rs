use crate::{
    models::mal_response::ParsedSong,
    utils::{
        redis::{check_cache, try_to_cache_response},
        statics::{SPOTIFY_CLIENT_ID, SPOTIFY_CLIENT_SECRET},
    },
};

use rspotify::{
    ClientCredsSpotify, Credentials,
    model::{Country, Market, SearchResult, SearchType},
    prelude::*,
};

use std::env;
use tracing::{error, info, instrument};

enum CacheLookup {
    Hit(Option<String>),
    Miss,
}

enum SearchOutcome {
    Found(String),
    NotFound,
    Failed,
}

trait SpotifyBackend {
    fn read_cache(&mut self, key: &str) -> CacheLookup;
    fn authenticate(&mut self) -> bool;
    fn search(&mut self, song_name: &str, artist_name: &str) -> SearchOutcome;
    fn write_cache(&mut self, key: &str, value: &str);
}

#[derive(Default)]
struct DefaultSpotifyBackend {
    client: Option<ClientCredsSpotify>,
}

impl SpotifyBackend for DefaultSpotifyBackend {
    fn read_cache(&mut self, key: &str) -> CacheLookup {
        match check_cache(key) {
            Ok(value) => {
                info!("Cache hit for {key:#?}");
                match value.as_str() {
                    "None" => CacheLookup::Hit(None),
                    _ => CacheLookup::Hit(Some(value)),
                }
            }
            Err(err) => {
                info!("Cache miss for {key:#?} with error {err:#?}");
                CacheLookup::Miss
            }
        }
    }

    fn authenticate(&mut self) -> bool {
        let spotify = get_spotify_client();
        if let Err(err) = spotify.request_token() {
            error!(error = %err, "Failed to request Spotify token");
            return false;
        }

        self.client = Some(spotify);
        true
    }

    fn search(&mut self, song_name: &str, artist_name: &str) -> SearchOutcome {
        let Some(spotify) = self.client.as_ref() else {
            return SearchOutcome::Failed;
        };

        match send_search_request(spotify, song_name, artist_name) {
            Ok(search_result) => match get_url_from_search_result(search_result) {
                Some(url) => SearchOutcome::Found(url),
                None => SearchOutcome::NotFound,
            },
            Err(err) => {
                info!(error = %err, "Could not find Spotify track");
                SearchOutcome::Failed
            }
        }
    }

    fn write_cache(&mut self, key: &str, value: &str) {
        try_to_cache_response(key, value);
    }
}

#[instrument(name = "spotify.get_client", skip_all)]
fn get_spotify_client() -> ClientCredsSpotify {
    let client_id =
        env::var(SPOTIFY_CLIENT_ID).expect("Expected a spotify client id in the environment");
    let client_secret = env::var(SPOTIFY_CLIENT_SECRET)
        .expect("Expected a spotify client secret in the environment");
    let credentials = Credentials {
        id: client_id,
        secret: Some(client_secret),
    };
    let spotify = ClientCredsSpotify::new(credentials);
    info!("Spotify client established");
    spotify
}

#[instrument(name = "spotify.send_search_request", skip(spotify, song_name, artist_name), fields(song = %song_name, artist = %artist_name))]
fn send_search_request(
    spotify: &ClientCredsSpotify,
    song_name: &str,
    artist_name: &str,
) -> Result<SearchResult, rspotify::ClientError> {
    // rspotify's blocking ureq client applies a 10-second deadline to each
    // token and API request.
    spotify.search(
        format!("track:{song_name} artist:{artist_name}").as_str(),
        SearchType::Track,
        Some(Market::Country(Country::UnitedStates)),
        None,
        Some(5),
        None,
    )
}

#[instrument(name = "spotify.extract_track_url", skip(search_result))]
fn get_url_from_search_result(search_result: SearchResult) -> Option<String> {
    if let SearchResult::Tracks(page) = search_result {
        // Gets URL for top result
        if !page.items.is_empty() {
            let track = &page.items[0];
            info!("Found track: {track:#?}");
            return Some(track.external_urls["spotify"].to_owned());
        }
        None
    } else {
        info!("Something else");
        None
    }
}

#[instrument(name = "spotify.search_song", skip(backend, kana_name, romaji_name, artist_name), fields(song = %romaji_name, artist = %artist_name))]
fn search_song<B: SpotifyBackend>(
    backend: &mut B,
    romaji_name: &str,
    kana_name: Option<&str>,
    artist_name: &str,
) -> Option<String> {
    match backend.search(romaji_name, artist_name) {
        SearchOutcome::Found(url) => Some(url),
        SearchOutcome::NotFound => match kana_name {
            Some(kana_name) => match backend.search(kana_name, artist_name) {
                SearchOutcome::Found(url) => Some(url),
                SearchOutcome::NotFound | SearchOutcome::Failed => None,
            },
            None => None,
        },
        SearchOutcome::Failed => None,
    }
}

#[instrument(name = "spotify.enrich_songs_backend", skip(songs, backend), fields(count = songs.len()))]
fn enrich_songs_with_backend<B: SpotifyBackend>(songs: &mut [ParsedSong], backend: &mut B) {
    let mut authentication = None;

    for song in songs.iter_mut() {
        let Some(artist) = song.artist_names.as_deref() else {
            continue;
        };

        // Keep the historical cache key format for compatibility.
        let cache_key = format!("{}:{:#?}:{}", song.romaji_name, song.kana_name, artist);
        if let CacheLookup::Hit(url) = backend.read_cache(&cache_key) {
            song.spotify_url = url;
            continue;
        }

        let authenticated = *authentication.get_or_insert_with(|| backend.authenticate());
        if !authenticated {
            // No Spotify lookup occurred, so do not turn a transient token
            // failure into a five-hour negative cache entry.
            song.spotify_url = None;
            continue;
        }

        let url = search_song(
            backend,
            &song.romaji_name,
            song.kana_name.as_deref(),
            artist,
        );

        backend.write_cache(&cache_key, url.as_deref().unwrap_or("None"));
        song.spotify_url = url;
    }
}

/// Fill in `spotify_url` for each [`ParsedSong`] that has an artist.
///
/// One client and access token are reused for all uncached songs and kana
/// fallbacks in this batch. This performs synchronous Spotify + Redis I/O and
/// is intended to run inside `spawn_blocking`.
#[instrument(name = "spotify.enrich_songs", skip(songs), fields(count = songs.len()))]
pub fn enrich_songs_with_spotify(songs: &mut [ParsedSong]) {
    enrich_songs_with_backend(songs, &mut DefaultSpotifyBackend::default());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, VecDeque};

    #[derive(Default)]
    struct FakeBackend {
        cache: HashMap<String, Option<String>>,
        cached_writes: Vec<(String, String)>,
        authentication_count: usize,
        authentication_result: Option<bool>,
        searches: Vec<(String, String)>,
        search_results: VecDeque<SearchOutcome>,
    }

    impl SpotifyBackend for FakeBackend {
        fn read_cache(&mut self, key: &str) -> CacheLookup {
            match self.cache.get(key) {
                Some(value) => CacheLookup::Hit(value.clone()),
                None => CacheLookup::Miss,
            }
        }

        fn authenticate(&mut self) -> bool {
            self.authentication_count += 1;
            self.authentication_result.unwrap_or(true)
        }

        fn search(&mut self, song_name: &str, artist_name: &str) -> SearchOutcome {
            self.searches
                .push((song_name.to_string(), artist_name.to_string()));
            self.search_results
                .pop_front()
                .unwrap_or(SearchOutcome::NotFound)
        }

        fn write_cache(&mut self, key: &str, value: &str) {
            self.cached_writes
                .push((key.to_string(), value.to_string()));
        }
    }

    fn song(romaji_name: &str, kana_name: Option<&str>, artist: &str) -> ParsedSong {
        ParsedSong {
            display_number: 1,
            song_name: romaji_name.to_string(),
            romaji_name: romaji_name.to_string(),
            kana_name: kana_name.map(str::to_string),
            artist_names: Some(artist.to_string()),
            episode_numbers: None,
            spotify_url: None,
        }
    }

    #[test]
    fn enrichment_authenticates_once_for_multiple_songs() {
        let mut songs = vec![
            song("First", None, "Artist A"),
            song("Second", None, "Artist B"),
        ];
        let mut backend = FakeBackend {
            search_results: VecDeque::from([
                SearchOutcome::Found("https://spotify/first".to_string()),
                SearchOutcome::Found("https://spotify/second".to_string()),
            ]),
            ..Default::default()
        };

        enrich_songs_with_backend(&mut songs, &mut backend);

        assert_eq!(backend.authentication_count, 1);
        assert_eq!(backend.searches.len(), 2);
        assert_eq!(
            songs[0].spotify_url.as_deref(),
            Some("https://spotify/first")
        );
        assert_eq!(
            songs[1].spotify_url.as_deref(),
            Some("https://spotify/second")
        );
    }

    #[test]
    fn kana_fallback_reuses_authenticated_client() {
        let mut songs = vec![song("Romaji", Some("かな"), "Artist")];
        let mut backend = FakeBackend {
            search_results: VecDeque::from([
                SearchOutcome::NotFound,
                SearchOutcome::Found("https://spotify/kana".to_string()),
            ]),
            ..Default::default()
        };

        enrich_songs_with_backend(&mut songs, &mut backend);

        assert_eq!(backend.authentication_count, 1);
        assert_eq!(
            backend.searches,
            [
                ("Romaji".to_string(), "Artist".to_string()),
                ("かな".to_string(), "Artist".to_string())
            ]
        );
        assert_eq!(
            songs[0].spotify_url.as_deref(),
            Some("https://spotify/kana")
        );
    }

    #[test]
    fn cache_hits_skip_authentication_and_preserve_negative_entries() {
        let mut songs = vec![
            song("Found", None, "Artist"),
            song("Missing", None, "Artist"),
        ];
        let mut backend = FakeBackend::default();
        backend.cache.insert(
            "Found:None:Artist".to_string(),
            Some("https://spotify/cached".to_string()),
        );
        backend
            .cache
            .insert("Missing:None:Artist".to_string(), None);

        enrich_songs_with_backend(&mut songs, &mut backend);

        assert_eq!(backend.authentication_count, 0);
        assert!(backend.searches.is_empty());
        assert_eq!(
            songs[0].spotify_url.as_deref(),
            Some("https://spotify/cached")
        );
        assert!(songs[1].spotify_url.is_none());
    }

    #[test]
    fn failed_lookup_does_not_prevent_later_songs_from_succeeding() {
        let mut songs = vec![
            song("First", None, "Artist"),
            song("Second", None, "Artist"),
        ];
        let mut backend = FakeBackend {
            search_results: VecDeque::from([
                SearchOutcome::Failed,
                SearchOutcome::Found("https://spotify/second".to_string()),
            ]),
            ..Default::default()
        };

        enrich_songs_with_backend(&mut songs, &mut backend);

        assert_eq!(backend.authentication_count, 1);
        assert!(songs[0].spotify_url.is_none());
        assert_eq!(
            songs[1].spotify_url.as_deref(),
            Some("https://spotify/second")
        );
        assert_eq!(
            backend.cached_writes[0],
            ("First:None:Artist".to_string(), "None".to_string())
        );
    }

    #[test]
    fn failed_authentication_does_not_write_negative_cache_entries() {
        let mut songs = vec![
            song("First", None, "Artist"),
            song("Second", None, "Artist"),
        ];
        let mut backend = FakeBackend {
            authentication_result: Some(false),
            ..Default::default()
        };

        enrich_songs_with_backend(&mut songs, &mut backend);

        assert_eq!(backend.authentication_count, 1);
        assert!(backend.searches.is_empty());
        assert!(backend.cached_writes.is_empty());
        assert!(songs.iter().all(|song| song.spotify_url.is_none()));
    }
}
