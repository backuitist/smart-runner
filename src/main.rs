extern crate termion;
extern crate itertools;

mod screen;
mod command;

use termion::event::Key;
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};
use std::io::{Write, stdin, stderr, Stderr};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use command::Command;
use screen::{Screen, ValidatedKeyword};

type Result<T> = std::result::Result<T, Box<std::error::Error>>;

fn main() {

    match run_runner() {
        Ok(Some(cmd)) => println!("{}", cmd),
        Ok(None)      => println!(), // needed when piped with read cmd
        Err(e)        => eprintln!("Error: {}", e)
    }
}


fn run_runner() -> Result<Option<String>> {

    // TODO have the commands stored externally
    let mut runner = Runner::new(
        vec![
            Command {
                cmd: "nix-env -q '.*{}.*'".to_owned(),
                description: Some("Search a Nix package by name".to_owned()),
                keywords: vec!["nix".to_owned(), "search".to_owned(), "package".to_owned()]
            },
            Command {
                cmd: "du -sh /nix/store".to_owned(),
                description: Some("Show the size of the Nix store".to_owned()),
                keywords: vec!["nix".to_owned(), "store".to_owned(), "size".to_owned()]
            },
            Command {
                cmd: "sudo shutdown -h now".to_owned(),
                description: Some("Shut the system down".to_owned()),
                keywords: vec!["hardware".to_owned(), "shutdown".to_owned()]
            }
        ])?;

    runner.run()
}


struct Runner {
    commands: Commands,
    screen: Screen,
    terminal: RawTerminal<Stderr> // we use stderr to not pollute stdout
}

enum InputLoopAction {
    Continue, Cancel, Success(String)
}

struct Commands {
    commands: Vec<Rc<Command>>,
    kwd2cmd: HashMap<String, HashSet<Rc<Command>>>
}

impl Commands {
    fn new(vec_commands: Vec<Command>) -> Commands {
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

impl Runner {
    fn new(vec_commands: Vec<Command>) -> Result<Runner> {
        let mut terminal = stderr().into_raw_mode()?;
        let screen = Screen::new(&mut terminal)?;
        let commands = Commands::new(vec_commands);

        Ok(Runner { commands, screen, terminal })
    }

    /// Return a command to execute or None if the user canceled
    fn run(self: &mut Runner) -> Result<Option<String>> {
        self.refresh_screen()?;
        let stdin = stdin();

        self.terminal.flush()?;
        for c in stdin.keys() {
            match self.process_key(c?) {
                InputLoopAction::Success(cmd) => {
                    self.cleanup()?;
                    return Ok(Some(cmd));
                },

                InputLoopAction::Cancel => {
                    self.cleanup()?;
                    return Ok(None);
                }

                InputLoopAction::Continue => {
                    self.refresh_screen()?;
                }
            };
        }

        unreachable!()
    }

    fn cleanup(self: &mut Runner) -> Result<()> {
        self.screen.cleanup(&mut self.terminal)
    }

    fn process_key(self: &mut Runner, key: Key) -> InputLoopAction {

        fn cont<F: FnOnce() -> ()>(f: F) -> InputLoopAction {
            f();
            InputLoopAction::Continue
        }

        match key {
            Key::Char('q') => InputLoopAction::Cancel,

            Key::Char('\n') => {
                if let Some(cmd) = self.screen.selected_command() {
                    InputLoopAction::Success(cmd.cmd.clone())
                } else {
                    InputLoopAction::Continue
                }
            },

            Key::Char('\t') => cont(|| self.auto_complete()),
            Key::Char(' ')  => cont(|| self.validate_keyword()),
            Key::Char(c)    => cont(|| self.add_key(c)),
            Key::Backspace  => cont(|| self.remove_last_char()),

            Key::Right      => cont(|| self.screen.next_suggestion()),

            Key::Left       => cont(|| self.screen.previous_suggestion()),
            Key::Up         => cont(|| self.screen.previous_command()),
            Key::Down       => cont(|| self.screen.next_command()),
            _               => cont(|| ())
        }
    }

    fn auto_complete(self: &mut Runner) {
        self.screen.complete();
        self.filter_commands();
    }

    fn add_key(self: &mut Runner, c: char) {
        self.screen.add(c);
        self.filter_commands();
    }

    fn remove_last_char(self: &mut Runner) {
        self.screen.remove_last_char();
        self.filter_commands();
    }

    fn filter_commands(self: &mut Runner) {
        let suggestion = filter_commands_with(
            &self.commands,
            self.screen.input().as_ref(),
            self.screen.validated_keywords.iter());

        self.screen.set_suggestion(suggestion);
    }

    fn validate_keyword(self: &mut Runner) {
        let input = self.screen.reset_input();
        let validated_kw = if self.commands.kwd2cmd.contains_key(&input) {
            ValidatedKeyword::Valid(input)
        } else {
            ValidatedKeyword::Invalid(input)
        };
        self.screen.add_validated_keyword(validated_kw);
    }

    fn refresh_screen(self: &mut Runner) -> Result<()> {
        self.screen.print(&mut self.terminal)
    }
}

fn filter_commands_with<'a, I>(commands: &Commands,
                               input: &str,
                               validated_keywords: I) -> screen::Suggestion
    where I: Iterator<Item=&'a ValidatedKeyword> {
    let mut suggestion: screen::Suggestion = Default::default();

    let validated_keywords: HashSet<String> = validated_keywords
        .filter_map(|v| match v {
            &ValidatedKeyword::Valid(ref kw) => Some(kw),
            _ => None
        }).cloned().collect();

    let validated_commands: HashSet<Rc<Command>> = if validated_keywords.is_empty() {
        commands.commands.iter().cloned().collect()
    } else {
        commands.commands.iter().cloned()
            .filter(|cmd| validated_keywords.iter()
                .all(|kw| cmd.keywords.contains(kw)))
            .collect()
    };

    if input.is_empty() {
        suggestion.commands.extend(validated_commands);
    } else {
        for (kw, cmds) in &commands.kwd2cmd {
            if kw.starts_with(&input)
                && !validated_keywords.contains(kw) {
                suggestion.keywords.push(kw.clone());
                suggestion.commands.extend(cmds.intersection(&validated_commands)
                    .cloned());
            }
        }
    }

    suggestion
}

//#[test]
//fn
