pub fn bold(input: String) -> String {
    format!("**{}**", input)
}

pub fn italics(input: String) -> String {
    format!("*{}*", input)
}

pub fn code(input: String) -> String {
    format!("`{}`", input)
}

pub fn strike(input: String) -> String {
    format!("~~{}~~", input)
}

pub fn linker(text: String, link: String) -> String {
    format!("[{}]({})", text, link)
}
