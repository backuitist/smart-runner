extern crate termion;
extern crate itertools;

use termion::{clear, color, cursor, style};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};
use std::io::{Write, stdin, stderr, Stderr};
use std::fmt::Write as FmtWrite;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

fn main() {

    match run_runner() {
        Ok(Some(cmd)) => println!("{}", cmd),
        Ok(None)      => println!(), // needed when piped with read cmd
        Err(e)        => eprintln!("Error: {}", e)
    }
}

type Result<T> = std::result::Result<T, Box<std::error::Error>>;

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
    commands: Vec<Rc<Command>>,
    screen: Screen,
    terminal: RawTerminal<Stderr>, // we use stderr to not pollute stdout
    kwd2cmd: HashMap<String, HashSet<Rc<Command>>>
}

enum InputLoopAction {
    Continue, Cancel, Success(String)
}

impl Runner {
    fn new(vec_commands: Vec<Command>) -> Result<Runner> {
        let mut stdout = stderr().into_raw_mode()?;
        let screen = Screen::new(&mut stdout)?;
        let commands: Vec<Rc<Command>> = vec_commands.into_iter().map(|cmd| Rc::new(cmd)).collect();
        let mut kwd2cmd: HashMap<String, HashSet<Rc<Command>>> = HashMap::new();
        for cmd in &commands {
            for kw in &cmd.keywords {
                let set = kwd2cmd.entry(kw.clone()).or_insert(HashSet::new());
                set.insert(cmd.clone());
            }
        }
        Ok(Runner { commands, screen, terminal: stdout, kwd2cmd })
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
        let input = self.screen.input();

        let mut keywords: Vec<String> = Vec::new();
        let mut eligible_commands: Vec<Rc<Command>> = Vec::new();

        let validated_keywords: HashSet<String> = self.screen.validated_keywords.iter()
            .filter_map(|v| match v {
                &ValidatedKeyword::Valid(ref kw) => Some(kw),
                _ => None
            }).cloned().collect();

        let validated_commands: HashSet<Rc<Command>> = if validated_keywords.is_empty() {
            self.commands.iter().cloned().collect()
        } else {
            self.commands.iter().cloned()
                .filter(|cmd| validated_keywords.iter()
                    .all(|kw| cmd.keywords.contains(kw)))
                .collect()
        };

        if input.is_empty() {
            eligible_commands.extend(validated_commands);
        } else {
            for (kw, cmds) in &self.kwd2cmd {
                if kw.starts_with(&input)
                    && !validated_keywords.contains(kw) {
                    keywords.push(kw.clone());
                    eligible_commands.extend(cmds.intersection(&validated_commands)
                        .cloned());
                }
            }
        }

        self.screen.set_commands(eligible_commands);
        self.screen.set_auto_complete(keywords);
    }

    fn validate_keyword(self: &mut Runner) {
        let input = self.screen.reset_input();
        let validated_kw = if self.kwd2cmd.contains_key(&input) {
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

enum ValidatedKeyword {
    Valid(String),
    Invalid(String)
}

// see below for an explanation of why this isn't a mere function
// Note: it's here because it has to be above it's application point
macro_rules! write_highlighted {
    ($dst:expr, $msg:expr, $bg_color:expr) =>
        (write!($dst, "{}{}{}{}{}",
               color::Bg($bg_color),
               color::Fg(color::Black),
               $msg,
               color::Bg(color::Reset),
               color::Fg(color::Reset)))
}

struct Screen {
    x: u16,
    y: u16,
    prompt: String,
    current_line: Vec<char>,
    validated_keywords: Vec<ValidatedKeyword>,
    auto_complete: Vec<String>,
    selected_auto_complete_index: Option<usize>,
    commands: Vec<Rc<Command>>,
    selected_command_index: Option<usize>,
    term_size: (u16,u16)
}


#[derive(Clone, Hash, Eq, PartialEq)]
struct Command {
    pub cmd: String,
    pub description: Option<String>,
    pub keywords: Vec<String>, // TODO should be a Set
}

impl Command {
    fn some_description<'a>(self: &'a Command) -> &'a str {
        self.description.as_ref().map_or("", String::as_str)
    }
}

impl Screen {
    fn new<T: Write>(stdout: &mut RawTerminal<T>) -> Result<Screen> {
        //let vertical_size: u16 = 6;
        //write!(stdout, "{}", "\n".repeat(vertical_size as usize))?;

        // Cannot get the cursor position without eating off one char
        // See https://github.com/ticki/termion/issues/136
        // let (_,y) = stdout.cursor_pos()?;

        let term_size = termion::terminal_size().unwrap_or((80, 10));
        let screen = Screen {
            prompt: "> ".to_owned(),
            x: 1,
            y: 1, // y - vertical_size,
            current_line: Vec::new(),
            validated_keywords: Vec::new(),
            auto_complete: Vec::new(),
            selected_auto_complete_index: None,
            selected_command_index: None,
            commands: Vec::new(),
            term_size
        };
        screen.print(stdout)?;

        Ok(screen)
    }

    fn cleanup<T: Write>(self: &mut Screen, terminal: &mut RawTerminal<T>) -> Result<()> {
        write!(terminal, "{}{}",
            cursor::Goto(self.x, self.y),
            clear::AfterCursor)?;
        terminal.flush()?;
        Ok(())
    }

    fn complete(self: &mut Screen) {
        if let Some(idx) = self.selected_auto_complete_index {
            self.validated_keywords.push(ValidatedKeyword::Valid(
                self.auto_complete.get(idx).unwrap().clone()));
            self.current_line = Vec::new();
        }
    }

    fn add_validated_keyword(self: &mut Screen, vkw: ValidatedKeyword) {
        self.validated_keywords.push(vkw);
    }

    fn selected_command(self: &Screen) -> Option<Rc<Command>> {
        self.selected_command_index.and_then(|idx|
            self.commands.get(idx).map(|cmd|cmd.clone()))
    }

    fn next_suggestion(self: &mut Screen) {
        if let Some(s) = self.selected_auto_complete_index {
            self.selected_auto_complete_index = Some((s + 1) % self.auto_complete.len());
        }
    }

    fn previous_suggestion(self: &mut Screen) {
        if let Some(s) = self.selected_auto_complete_index {
            self.selected_auto_complete_index = Some(if s == 0 { self.auto_complete.len() - 1 } else { s - 1 });
        }
    }

    fn next_command(self: &mut Screen) {
        if let Some(s) = self.selected_command_index {
            self.selected_command_index = Some((s + 1) % self.commands.len());
        }
    }

    fn previous_command(self: &mut Screen) {
        if let Some(s) = self.selected_command_index {
            self.selected_command_index = Some(if s == 0 { self.commands.len() - 1 } else { s - 1 });
        }
    }

    fn set_auto_complete(self: &mut Screen, keywords: Vec<String>) {
        self.auto_complete = keywords;
        if self.auto_complete.is_empty() {
            self.selected_auto_complete_index = None;
        } else {
            self.selected_auto_complete_index = Some(0);
        }
    }

    fn set_commands(self: &mut Screen, commands: Vec<Rc<Command>>) {
        self.commands = commands;
        if self.commands.is_empty() {
            self.selected_command_index = None;
        } else {
            self.selected_command_index = Some(0);
        }
    }

    fn input(self: &Screen) -> String {
        self.current_line.iter().cloned().collect()
    }

    /// return the previous input
    fn reset_input(self: &mut Screen) -> String {
        std::mem::replace(&mut self.current_line, Vec::new()).into_iter().collect()
    }

    fn add(self: &mut Screen, key: char) {
        self.current_line.push(key);
    }

    fn remove_last_char(self: &mut Screen) {
        if self.current_line.is_empty() {
            self.validated_keywords.pop();
        } else {
            self.current_line.pop();
        }
    }

    fn print<T: Write>(self: &Screen, terminal: &mut RawTerminal<T>) -> Result<()> {

        let auto_complete_string = if let Some(selection) = self.selected_auto_complete_index {
            let mut new_item = String::new();
            let mut ac: Vec<&String> = self.auto_complete.iter().collect();

            let replace_selection = ac.get(selection).map(|item| {
                write_highlighted!(new_item, item, color::Yellow) // TODO we're not doing anything with the Result
            }).is_some();

            if replace_selection {
                ac.remove(selection);
                ac.insert(selection, &new_item)
            }

            ac.iter().join(" ")
        } else { "".to_owned() };


        write!(terminal, "{}{}",
               cursor::Goto(1, self.y + 1),
               "─".repeat(self.term_size.0 as usize))?;

        write!(terminal, "{}{}",
               cursor::Goto(1, self.y + 3),
               "─".repeat(self.term_size.0 as usize))?;

        write!(terminal, "{}{}{}",
               cursor::Goto(1, self.y + 2),
               clear::CurrentLine,
               auto_complete_string
        )?;

        // print commands
        write!(terminal, "{}{}",
            termion::cursor::Goto(1, self.y + 4),
            termion::clear::AfterCursor)?;

        for (i,cmd) in self.commands.iter().enumerate() {
            let description = colorize_fg(cmd.some_description(), color::Green);

            match self.selected_command_index {
                Some(sel) if i == sel =>
                    writeln!(terminal, "{}{} {}{}\r",
                             style::Bold,
                             cmd.cmd,
                             description,
                             style::Reset)?,
                _ =>
                    writeln!(terminal, "{} {}\r", cmd.cmd, description)?
            };
        }

        write!(terminal, "{}{}{}",
               termion::cursor::Goto(self.x, self.y),
               clear::CurrentLine,
               self.prompt)?;

        for vk in &self.validated_keywords {
            match vk {
                &ValidatedKeyword::Valid(ref kw) =>
                    write_highlighted!(terminal, kw, color::Green)?,

                &ValidatedKeyword::Invalid(ref kw) =>
                    write_highlighted!(terminal, kw, color::Red)?

            };
            write!(terminal, " ")?;
        }

        let line: String = self.input();
        write!(terminal, "{}", line)?;

        terminal.flush()?;
        Ok(())
    }
}

fn colorize_fg<C: color::Color>(msg: &str, color: C) -> String {
    let mut colorized = String::new();
    write!(colorized, "{}{}{}", color::Fg(color), msg, color::Fg(color::Reset));
    colorized
}


// There's currently no way of providing write_highlighted through traits as
// it is impossible to provide an implementation for both Write and FmtWrite.
// Although the 2 traits share a lot of similarities, FmtWrite takes UTF-8 formatted
// Strings and discards errors, whereas Write takes [u8] and reports errors.
// Those differences result in a missing "bridge" between the two: FmtWrite does
// not have a Write implementation, and neither has Write an FmtWrite implementation.
// This is probably the reason why write! and writeln! are macros.
//
//pub trait WriteExt {
//    fn write_highlighted<C: color::Color>(self: &mut Self,
//                                          msg: &str,
//                                          bg_color: C) -> Result<()>;
//}
//
//impl<W: Write> WriteExt for W {
//    fn write_highlighted<C: color::Color>(self: &mut W,
//                                          msg: &str,
//                                          bg_color: C) -> Result<()> {
//        write!(self, "{}{}{}{}{}",
//               color::Bg(bg_color),
//               color::Fg(color::Black),
//               msg,
//               color::Bg(color::Reset),
//               color::Fg(color::Reset))
//    }
//}
//
//
// Another option would have been to specialize for String, knowing
// that the `write_highlighted` implementation for Write would produce
// correctly formatted UTF-8 Vec<u8>.
// BUT specialization hasn't landed yet...
//
//impl WriteExt for String {
//    fn write_highlighted<C: color::Color>(self: &mut String,
//                                          msg: &str,
//                                          bg_color: C) -> Result<()> {
//        let v: Vec<u8> = Vec::new(); // Vec<u8> is Write
//        v.write_highlighted(msg, bg_color)?;
//        // write_highlighted produces UTF-8
//        unsafe {
//            write!(self, "{}", String::from_utf8_unchecked(v))
//        }
//        Ok(())
//    }
//}