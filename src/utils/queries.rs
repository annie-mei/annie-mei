pub const FETCH_ANILIST_USER: &str = "
query ($username: String) { 
  User (name: $username) { 
    id 
  } 
}
";

pub const FETCH_USER_MEDIA_LIST_DATA: &str = "
query ($userId: Int, $type: MediaType, $mediaId: Int) {
  MediaList(userId: $userId, type: $type, mediaId: $mediaId) {
    status
    score(format: POINT_100)
    progress
    progressVolumes
    media{
      episodes
      volumes
    }
  }
}
";
