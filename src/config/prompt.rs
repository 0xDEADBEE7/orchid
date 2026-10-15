//! Terminal prompts for `orchid auth add`. Questions go to stderr (stdout stays
//! JSON) and are asked only when stdin and stderr are terminals; flags answer
//! them otherwise.
use std::{
    io::{self, IsTerminal, Write},
    process::{Command, Stdio},
};

pub(super) struct Prompter<'a> {
    pub args: &'a [String],
    pub interactive: bool,
}

impl<'a> Prompter<'a> {
    pub fn new(args: &'a [String]) -> Self {
        let interactive = io::stdin().is_terminal() && io::stderr().is_terminal();
        Self { args, interactive }
    }

    /// `flag`'s value, else (on a terminal) the answer, else `default`.
    pub fn ask(&self, flag: &str, question: &str, default: &str) -> io::Result<String> {
        if let Some(value) = flag_value(self.args, flag) {
            return Ok(value);
        }
        if !self.interactive {
            return Ok(default.into());
        }
        let shown = match default {
            "" => String::new(),
            default => format!(" [{default}]"),
        };
        let answer = read_line(&format!("{question}{shown}: "))?;
        Ok(if answer.is_empty() {
            default.into()
        } else {
            answer
        })
    }

    /// A yes/no question on a terminal; `default` otherwise.
    pub fn confirm(&self, question: &str, default: bool) -> io::Result<bool> {
        let hint = if default { "Y/n" } else { "y/N" };
        let answer = self.ask("", &format!("{question} ({hint})"), "")?;
        Ok(answer.is_empty() && default || answer.starts_with(['y', 'Y']))
    }
}

/// Asks on stderr and reads one line from stdin.
pub(super) fn read_line(question: &str) -> io::Result<String> {
    eprint!("{question}");
    io::stderr().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_owned())
}

/// Reads a line without echoing it.
pub(super) fn read_secret(question: &str) -> io::Result<String> {
    let stty = |flag: &str| {
        Command::new("stty")
            .arg(flag)
            .stdin(Stdio::inherit())
            .status()
    };
    stty("-echo")?;
    let secret = read_line(question);
    let _ = stty("echo");
    eprintln!();
    secret
}

/// Returns the value after `flag`, if present.
pub(super) fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}

/// Creates an invalid-input error.
pub(super) fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
