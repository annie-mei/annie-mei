use titlecase::titlecase as imported_titlecase;

pub fn bold(input: String) -> String {
    format!("**{}**", input)
}

pub fn italics(input: String) -> String {
    format!("*{}*", input)
}

pub fn code(input: String) -> String {
    format!("`{}`", input)
}

#[allow(dead_code)]
pub fn strike(input: String) -> String {
    format!("~~{}~~", input)
}

pub fn linker(text: String, link: String) -> String {
    format!("[{}]({})", text, link)
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
