use std::fmt::Debug;
use std::fmt::Display;
use std::io;
use std::io::BufRead;
use std::io::IsTerminal;
use std::io::Stdin;
use std::io::StdinLock;
use std::io::Write;

use codespan_reporting::term;
use color_eyre::eyre;
use termcolor::Color;
use termcolor::ColorChoice;
use termcolor::StandardStream;
use termcolor::StandardStreamLock;
use termcolor::WriteColor;
use tytanic_core::test::Ident;
use tytanic_utils::cwrite;
use tytanic_utils::ui::Indented;

/// A terminal ui wrapper for common tasks such as input prompts and output
/// messaging.
#[derive(Debug)]
pub struct Ui {
    /// The unlocked stdin stream.
    stdin: Stdin,

    /// The unlocked stdout stream.
    stdout: StandardStream,

    /// The unlocked stderr stream.
    stderr: StandardStream,

    /// The diagnostic config to use for emitting typst source diagnostics.
    diagnostic_config: term::Config,
}

/// Returns whether or not a given output stream is connected to a terminal.
pub fn check_terminal<T: IsTerminal>(t: T, choice: ColorChoice) -> ColorChoice {
    match choice {
        // When we use auto and the stream is not a terminal, we disable it
        // since termcolor does not check for this, in any other case we let
        // termcolor figure out what to do.
        ColorChoice::Auto if !t.is_terminal() => ColorChoice::Never,
        other => other,
    }
}

impl Ui {
    /// Creates a new [`Ui`] with the gven color choices for stdout and stderr.
    pub fn new(out: ColorChoice, err: ColorChoice, diagnostic_config: term::Config) -> Self {
        Self {
            stdin: io::stdin(),
            stdout: StandardStream::stdout(check_terminal(io::stdout(), out)),
            stderr: StandardStream::stderr(check_terminal(io::stderr(), err)),
            diagnostic_config,
        }
    }
}

impl Ui {
    /// Whether a live status report can be printed and cleared using ANSI
    /// escape codes.
    pub fn can_live_report(&self) -> bool {
        io::stderr().is_terminal()
    }

    /// Whether a prompt can be displayed and confirmed by the user.
    pub fn can_prompt(&self) -> bool {
        io::stdin().is_terminal() && io::stderr().is_terminal()
    }

    /// Returns the diagnostic config to use for displaying diagnostics.
    pub fn diagnostic_config(&self) -> &term::Config {
        &self.diagnostic_config
    }

    /// Returns an exclusive lock to stdin.
    pub fn stdin(&self) -> StdinLock<'_> {
        self.stdin.lock()
    }

    /// Returns an exclusive lock to stdout.
    pub fn stdout(&self) -> StandardStreamLock<'_> {
        self.stdout.lock()
    }

    /// Returns an exclusive lock to stderr.
    pub fn stderr(&self) -> StandardStreamLock<'_> {
        self.stderr.lock()
    }
}

impl Ui {
    /// Returns a writer for emitting a user-facing error.
    pub fn error(&self) -> io::Result<Indented<impl WriteColor + '_>> {
        error(self.stderr())
    }

    /// Returns a writer for emitting a user-facing warning.
    pub fn warn(&self) -> io::Result<Indented<impl WriteColor + '_>> {
        warn(self.stderr())
    }

    /// Returns a writer for emitting a user-facing hint.
    pub fn hint(&self) -> io::Result<Indented<impl WriteColor + '_>> {
        hint(self.stderr())
    }

    /// Prompts the user for input with the given prompt on stderr.
    pub fn prompt_with(
        &self,
        prompt: impl FnOnce(&mut dyn WriteColor) -> io::Result<()>,
    ) -> eyre::Result<String> {
        if !self.can_prompt() {
            eyre::bail!(io::Error::new(
                io::ErrorKind::Unsupported,
                "Cannot prompt for input since the output is not connected to a terminal",
            ));
        }

        let mut stderr = self.stderr();
        let mut stdin = self.stdin();

        prompt(&mut stderr)?;
        stderr.flush()?;

        let mut buffer = String::new();
        stdin.read_line(&mut buffer)?;

        let trimmed = buffer.trim();
        if trimmed.is_empty() {
            eyre::bail!(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Prompt cancelled by EOF",
            ));
        }

        Ok(trimmed.to_owned())
    }

    /// A shorthand for [`Ui::prompt_with`] for confirmations.
    pub fn prompt_yes_no(
        &self,
        prompt: impl Display,
        default: impl Into<Option<bool>>,
    ) -> eyre::Result<bool> {
        let default = default.into();
        let def = match default {
            Some(true) => "Y/n",
            Some(false) => "y/N",
            None => "y/n",
        };

        let res = self.prompt_with(|err| write!(err, "{prompt} [{def}]: "))?;

        Ok(match &res[..] {
            "" => default.ok_or_else(|| eyre::eyre!("expected [y]es or [n]o, got nothing"))?,
            "y" | "Y" => true,
            "n" | "N" => false,
            _ => {
                if res.eq_ignore_ascii_case("yes") {
                    true
                } else if res.eq_ignore_ascii_case("no") {
                    false
                } else {
                    eyre::bail!("expected [y]es or [n]o, got: {res:?}");
                }
            }
        })
    }

    /// Flushes and resets both output streams.
    pub fn flush(&self) -> io::Result<()> {
        let mut out = self.stdout();
        let mut err = self.stderr();

        out.reset()?;
        write!(out, "")?;

        err.reset()?;
        write!(err, "")?;

        Ok(())
    }
}

/// Returns a writer for annotated output. Annotated output is output which uses
/// a hanging indent after an initial indentation. The writer will continue on
/// the same line as the annotation.
pub fn annotated<W: WriteColor>(
    mut w: W,
    header: &str,
    color: Color,
    max_align: impl Into<Option<usize>>,
) -> io::Result<Indented<W>> {
    let align = max_align.into().unwrap_or(header.len());
    cwrite!(bold_colored(w, color), "{header:>align$} ")?;

    // When taking the indent from the header length, we need to account for the
    // additional space.
    Ok(Indented::continued(w, align + 1))
}

/// Returns a writer for emitting a user-facing error.
pub fn error<W: WriteColor>(w: W) -> io::Result<Indented<W>> {
    annotated(w, "error:", Color::Red, None)
}

/// Returns a writer for emitting a user-facing warning.
pub fn warn<W: WriteColor>(w: W) -> io::Result<Indented<W>> {
    annotated(w, "warning:", Color::Yellow, None)
}

/// Returns a writer for emitting a user-facing hint.
pub fn hint<W: WriteColor>(w: W) -> io::Result<Indented<W>> {
    annotated(w, "hint:", Color::Cyan, None)
}

/// Write a test id.
pub fn write_test_ident(mut w: &mut dyn WriteColor, id: &Ident) -> io::Result<()> {
    // if !id.module().is_empty() {
    //     cwrite!(colored(w, Color::Cyan), "{}/", id.module())?;
    // }

    // cwrite!(bold_colored(w, Color::Blue), "{}", id.name())?;

    // Ok(())

    todo!();
}
