// @generated automatically by Diesel CLI.

diesel::table! {
    users (discord_id) {
        discord_id -> Int8,
        anilist_id -> Int8,
        anilist_username -> Text,
        access_token -> Nullable<Text>,
    }
}
