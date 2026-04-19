use titlecase::titlecase as imported_titlecase;

pub fn bold(input: &str) -> String {
    format!("**{input}**")
}

pub fn italics(input: &str) -> String {
    format!("*{input}*")
}

pub fn code(input: &str) -> String {
    format!("`{input}`")
}

#[allow(dead_code)]
pub fn strike(input: &str) -> String {
    format!("~~{input}~~")
}

pub fn linker(text: &str, link: &str) -> String {
    format!("[{text}]({link})")
}

pub fn remove_underscores_and_titlecase(text: &str) -> String {
    match text {
        "TV" | "OVA" | "ONA" => text.to_string(),
        _ => titlecase(&text.split('_').collect::<Vec<&str>>().join(" ")),
    }
}

pub fn titlecase(text: &str) -> String {
    imported_titlecase(text)
}
