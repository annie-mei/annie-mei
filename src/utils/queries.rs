pub const FETCH_ANILIST_USER: &str = "
query ($username: String) { 
  User (name: $username) { 
    id 
  } 
}
";
