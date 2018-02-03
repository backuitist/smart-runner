
use std::rc::Rc;
use std::collections::{HashMap, HashSet};

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

pub struct Commands {
    pub commands: Vec<Rc<Command>>,
    pub kwd2cmd: HashMap<String, HashSet<Rc<Command>>>
}

impl Commands {
    pub fn new(vec_commands: Vec<Command>) -> Commands {
        let commands: Vec<Rc<Command>> = vec_commands.into_iter().map(|cmd| Rc::new(cmd)).collect();
        let mut kwd2cmd: HashMap<String, HashSet<Rc<Command>>> = HashMap::new();
        for cmd in &commands {
            for kw in &cmd.keywords {
                let set = kwd2cmd.entry(kw.clone()).or_insert(HashSet::new());
                set.insert(cmd.clone());
            }
        }

        Commands { commands, kwd2cmd }
    }
}
