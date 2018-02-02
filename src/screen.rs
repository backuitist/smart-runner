extern crate termion;

use termion::{clear, color, cursor, style};
use termion::raw::RawTerminal;
use std::rc::Rc;
use std::io::Write;
use command::Command;
use std::fmt::Write as FmtWrite;
use itertools::Itertools;

type Result<T> = ::std::result::Result<T, Box<::std::error::Error>>;


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

pub struct Screen {
    x: u16,
    y: u16,
    prompt: String,
    current_line: Vec<char>,
    pub validated_keywords: Vec<ValidatedKeyword>,
    auto_complete: Vec<String>,
    selected_auto_complete_index: Option<usize>,
    commands: Vec<Rc<Command>>,
    selected_command_index: Option<usize>,
    term_size: (u16,u16)
}


#[derive(Default)]
pub struct Suggestion {
    pub keywords: Vec<String>,
    pub commands: Vec<Rc<Command>>,
}


pub enum ValidatedKeyword {
    Valid(String),
    Invalid(String)
}

impl Screen {
    pub fn new<T: Write>(stdout: &mut RawTerminal<T>) -> Result<Screen> {
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

    pub fn cleanup<T: Write>(self: &mut Screen, terminal: &mut RawTerminal<T>) -> Result<()> {
        write!(terminal, "{}{}",
               cursor::Goto(self.x, self.y),
               clear::AfterCursor)?;
        terminal.flush()?;
        Ok(())
    }

    pub fn complete(self: &mut Screen) {
        if let Some(idx) = self.selected_auto_complete_index {
            self.validated_keywords.push(ValidatedKeyword::Valid(
                self.auto_complete.get(idx).unwrap().clone()));
            self.current_line = Vec::new();
        }
    }

    pub fn add_validated_keyword(self: &mut Screen, vkw: ValidatedKeyword) {
        self.validated_keywords.push(vkw);
    }

    pub fn selected_command(self: &Screen) -> Option<Rc<Command>> {
        self.selected_command_index.and_then(|idx|
            self.commands.get(idx).map(|cmd|cmd.clone()))
    }

    pub fn next_suggestion(self: &mut Screen) {
        if let Some(s) = self.selected_auto_complete_index {
            self.selected_auto_complete_index = Some((s + 1) % self.auto_complete.len());
        }
    }

    pub fn previous_suggestion(self: &mut Screen) {
        if let Some(s) = self.selected_auto_complete_index {
            self.selected_auto_complete_index = Some(if s == 0 { self.auto_complete.len() - 1 } else { s - 1 });
        }
    }

    pub fn next_command(self: &mut Screen) {
        if let Some(s) = self.selected_command_index {
            self.selected_command_index = Some((s + 1) % self.commands.len());
        }
    }

    pub fn previous_command(self: &mut Screen) {
        if let Some(s) = self.selected_command_index {
            self.selected_command_index = Some(if s == 0 { self.commands.len() - 1 } else { s - 1 });
        }
    }

    pub fn set_suggestion(self: &mut Screen, suggestion: Suggestion) {
        self.set_commands(suggestion.commands);
        self.set_auto_complete(suggestion.keywords);
    }

    pub fn set_auto_complete(self: &mut Screen, keywords: Vec<String>) {
        self.auto_complete = keywords;
        if self.auto_complete.is_empty() {
            self.selected_auto_complete_index = None;
        } else {
            self.selected_auto_complete_index = Some(0);
        }
    }

    pub fn set_commands(self: &mut Screen, commands: Vec<Rc<Command>>) {
        self.commands = commands;
        if self.commands.is_empty() {
            self.selected_command_index = None;
        } else {
            self.selected_command_index = Some(0);
        }
    }

    pub fn input(self: &Screen) -> String {
        self.current_line.iter().cloned().collect()
    }

    /// return the previous input
    pub fn reset_input(self: &mut Screen) -> String {
        ::std::mem::replace(&mut self.current_line, Vec::new()).into_iter().collect()
    }

    pub fn add(self: &mut Screen, key: char) {
        self.current_line.push(key);
    }

    pub fn remove_last_char(self: &mut Screen) {
        if self.current_line.is_empty() {
            self.validated_keywords.pop();
        } else {
            self.current_line.pop();
        }
    }

    pub fn print<T: Write>(self: &Screen, terminal: &mut RawTerminal<T>) -> Result<()> {

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

        write!(terminal, "{}", self.input())?;

        terminal.flush()?;
        Ok(())
    }
}

fn colorize_fg<C: color::Color>(msg: &str, color: C) -> String {
    let mut colorized = String::new();
    write!(colorized, "{}{}{}", color::Fg(color), msg, color::Fg(color::Reset));
    colorized
}

// Note on the `write_highlighted` macro
//
// There's currently no way of providing `write_highlighted` through traits as
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