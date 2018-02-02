

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct Command {
    pub cmd: String,
    pub description: Option<String>,
    pub keywords: Vec<String>, // TODO should be a Set
}

impl Command {
    pub fn some_description<'a>(self: &'a Command) -> &'a str {
        self.description.as_ref().map_or("", String::as_str)
    }
}