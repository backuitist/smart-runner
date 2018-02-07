
use std::rc::Rc;
use std::collections::{HashMap, HashSet};
use std::cmp::Ordering;
use itertools::Itertools;

type Result<T> = ::std::result::Result<T, Box<::std::error::Error>>;

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct Command {
    pub cmd: Placeholders,
    pub description: Option<String>,
    pub keywords: Vec<String>, // TODO should be a Set
}

impl Ord for Command {
    fn cmp(&self, other: &Command) -> Ordering {
        self.cmd.original.cmp(&other.cmd.original)
    }
}

impl PartialOrd for Command {
    fn partial_cmp(&self, other: &Command) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Hash, Eq, PartialEq, Debug, Default)]
pub struct Placeholders {
    original: String,
    cmd_chunks: Vec<String>,
    names: Vec<String>
}



impl Placeholders {

    /// Syntax is: `my-command {placeholder name} -i {other}`
    pub fn parse(cmd: &str) -> Result<Placeholders> {
        use ::regex::{Regex};

        // TODO do not rebuild this for every command
        let regex = Regex::new(r"([^{]*)(\{([^}]*?)\})?")?;

        let mut placeholders = Placeholders {
            original: cmd.to_owned(),
            ..Default::default()
        };

        for capture in regex.captures_iter(cmd) {
            if let Some(cmd) = capture.get(1) {
                placeholders.cmd_chunks.push(cmd.as_str().to_owned())
            };
            if let Some(name) = capture.get(3) {
                placeholders.names.push(name.as_str().to_owned())
            };
        }

        Ok(placeholders)
    }

    pub fn interpolate(self: &Placeholders, values: Vec<String>) -> String {
        self.cmd_chunks.iter().interleave(values.iter()).join("")
    }
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
        Commands::new_rc(commands)
    }

    pub fn new_rc(commands: Vec<Rc<Command>>) -> Commands {
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

#[test]
fn parsing_placeholders_name_and_no_name() {
    let ph = Placeholders::parse("nix-env -q '.*{}.*'{name} blabla").unwrap();
    assert_eq!(ph.cmd_chunks, vec!["nix-env -q '.*", ".*'", " blabla"]);
    assert_eq!(ph.names, vec!["", "name"]);
}

#[test]
fn interpolating_placeholders() {
    let ph = Placeholders::parse("nix-env -q '.*{}.*'{name} blabla").unwrap();
    assert_eq!(ph.interpolate(vec!["stuff".to_owned(), "more-stuff".to_owned()]), "nix-env -q '.*stuff.*'more-stuff blabla");
}

