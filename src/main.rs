extern crate termion;
extern crate itertools;
extern crate regex;

#[cfg(test)] #[macro_use] extern crate hamcrest;
#[cfg(test)] #[macro_use] extern crate maplit; // provide `hashset!`

mod screen;
mod command;
mod suggestion;

use termion::event::Key;
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};
use std::io::{Write, stdin, stderr, Stderr};
use std::collections::HashSet;

use command::{Command, Commands, Placeholders};
use screen::{Screen, ValidatedKeyword};
use suggestion::Suggestion;

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
                cmd: Placeholders::parse("nix-env -q '.*{name}.*'")?,
                description: Some("Search a Nix package by name".to_owned()),
                keywords: vec!["nix".to_owned(), "search".to_owned(), "package".to_owned()]
            },
            Command {
                cmd: Placeholders::parse("du -sh /nix/store")?,
                description: Some("Show the size of the Nix store".to_owned()),
                keywords: vec!["nix".to_owned(), "store".to_owned(), "size".to_owned()]
            },
            Command {
                cmd: Placeholders::parse("sudo shutdown -h now")?,
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
                    InputLoopAction::Success(cmd)
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

        let suggestion = {
            // nest `validated_keywords` as it borrows self immutably
            let validated_keywords: HashSet<&String> = self.screen.validated_keywords.iter()
                .filter_map(|v| match v {
                    &ValidatedKeyword::Valid(ref kw) => Some(kw),
                    _ => None
                }).collect();

            Suggestion::from_input(
                &self.commands,
                self.screen.input().as_ref(),
                validated_keywords)
        };

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