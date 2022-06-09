//! # Clytia
//! Clytia is a helper library to make writing CLIs easier and smoother
//!
//! Clytia provides several functions, which either take in some form of user input
//! or allow you to provide feedback to the user while some process is ongoing.
//!
//! Look at the [`Clytia`] struct for more information.

#![deny(missing_docs)]
#![cfg_attr(feature = "nightly", feature(scoped_threads))]

use std::{
    collections::HashSet,
    io::{self, Read, Stdin, Stdout, Write},
    str::FromStr,
    sync::atomic::AtomicBool,
    time::Duration,
};

use crossbeam::thread::scope;
use owo_colors::OwoColorize;
use termion::{event::Key, input::TermRead, raw::IntoRawMode};

/// A alias for [`std::result::Result`] where the default error is [`Error`]
pub type Result<T, E = Error> = core::result::Result<T, E>;

static SPINNER_SYMBOLS: [char; 8] = ['⠹', '⢸', '⣰', '⣤', '⣆', '⡇', '⠏', '⠛'];

/// Clytia's Error type
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Represents an error where the user picked nothing, but input was required.
    /// See: [`parsed_input`]
    #[error("non optional input, but no input given")]
    NonOptionalInput,
    /// Represents a generic IO error, see: [`std::io::Error`]
    #[error("IO Error: {0}")]
    Io(#[from] io::Error),
    /// Represent a error where a given input could not be parsed correctly
    #[error("Could not parse: {0}")]
    ParseError(String),
}

/// Holder for an input an output, useful if you need custom buffer to read and write to.
/// For most cases, you can use [`Default::default`] e.g.
/// ```rust
/// use clytia::Clytia;
///
/// let cli = Clytia::default();
/// ```
#[derive(Debug)]
pub struct Clytia<I: Read, O: Write> {
    input: I,
    output: O,
}

impl<I: Read, O: Write> Drop for Clytia<I, O> {
    fn drop(&mut self) {
        write!(self.output, "\r{}", termion::cursor::Show).unwrap();
    }
}

impl<I: Read, O: Write> Clytia<I, O> {
    /// Create a new [`Clytia`] for a given input and output stream.
    pub fn new(input: I, output: O) -> Self {
        Self { input, output }
    }

    /// Get a reference to the input stream.
    pub fn input(&self) -> &I {
        &self.input
    }

    /// Get a reference to the output stream.
    pub fn output(&self) -> &O {
        &self.output
    }

    /// Get a mutable reference to the input stream.
    pub fn input_mut(&mut self) -> &mut I {
        &mut self.input
    }

    /// Get a mutable reference to the output stream.
    pub fn output_mut(&mut self) -> &mut O {
        &mut self.output
    }

    /// Get input from the user with an optional default.
    /// Takes in a given prompt and optionally a default value to use if nothing is inputted.
    ///
    /// # Result
    /// Returns:
    /// - [`Error::NonOptionalInput`] if `default` is [`None`] but the user didn't input anything
    /// - [`Error::Io`] if there is a problem reading/writing from stdin/stdout.
    /// - [`Error::ParseError`] if the given input could not be parsed to type `T`.
    /// - A `T` if the given input could be parsed.
    /// - `default` if set and no input is given.
    ///
    /// # Usage
    /// ## Without default value:
    /// ```rust
    /// use clytia::{Clytia, Error, Result};
    ///
    /// let mut cli = Clytia::default();
    ///
    /// let r: Result<usize> = cli.parsed_input("Please enter a number", None);
    ///
    /// match r {
    ///     Ok(r) => println!("Your number is: {}!", r),
    ///     Err(Error::NonOptionalInput) => println!("You didn't enter a number! 😠"),
    ///     Err(_) => println!("Uh oh...")
    /// }
    /// ```
    /// ## With default value:
    /// ```rust
    /// use clytia::{Clytia, Error, Result};
    ///
    /// let mut cli = Clytia::default();
    ///
    /// let r: Result<usize> = cli.parsed_input("Please enter a number", Some(1));
    ///
    /// match r {
    ///     Ok(r) => println!("Your number is: {}!", r),
    ///     Err(Error::NonOptionalInput) => unreachable!("You'll get Ok(1) instead."),
    ///     Err(_) => println!("Uh oh...")
    /// }
    /// ```
    pub fn parsed_input<S, T>(&mut self, prompt: S, default: Option<T>) -> Result<T>
    where
        S: std::fmt::Display,
        T: FromStr,
        T: std::fmt::Display,
    {
        let input_stream = &mut self.input;
        let output_stream = &mut self.output;

        match &default {
            Some(d) => {
                write!(
                    output_stream,
                    "{}{}{}",
                    prompt.blue(),
                    format!("(default: {})", d).magenta(),
                    " => ".blue()
                )?;
            }
            None => {
                write!(output_stream, "{}", format!("{} => ", prompt).blue())?;
            }
        };
        output_stream.flush()?;

        let input = input_stream.read_line()?;
        let ret = match input {
            None => match default {
                Some(v) => Ok(v),
                None => Err(Error::NonOptionalInput),
            },
            Some(n) if n.trim().trim_end().is_empty() => match default {
                Some(v) => Ok(v),
                None => Err(Error::NonOptionalInput),
            },
            Some(n) => {
                let trimmed = n.trim().trim_end();
                trimmed
                    .parse()
                    .map_err(|_| Error::ParseError(trimmed.to_string()))
            }
        }?;

        Ok(ret)
    }

    /// Get input from the user with a custom validation function.
    /// Takes in a given prompt and a validation function that is used to check the input before returning.
    ///
    /// # Usage
    /// ```rust
    /// use clytia::{Clytia, Error, Result};
    ///
    /// let mut cli = Clytia::default();
    ///
    /// let r = usize = cli.validated_input("Please enter a number", "0-10", |n| n >= 0 && n <= 10).unwrap();
    /// assert!(r >= 0 && r <= 10);
    /// ```
    pub fn validated_input<T, S, R, F>(
        &mut self,
        prompt: S,
        requirements: R,
        validate: F,
    ) -> Result<T>
    where
        S: std::fmt::Display,
        R: std::fmt::Display,
        T: FromStr,
        F: Fn(&T) -> bool,
    {
        let input_stream = &mut self.input;
        let output_stream = &mut self.output;

        loop {
            write!(
                output_stream,
                "{}\r{} {} {} ",
                termion::clear::CurrentLine,
                prompt.blue(),
                format!("(requirements: {})", requirements).magenta(),
                "=>".blue()
            )?;
            output_stream.flush()?;

            let input = input_stream.read_line()?;
            let r = match input {
                None => {
                    write!(
                        output_stream,
                        "{}{}\r",
                        termion::cursor::Up(1),
                        termion::clear::CurrentLine,
                    )?;
                    write!(
                        output_stream,
                        "{} {} {} ",
                        prompt.red(),
                        format!("(requirements: {})", requirements).magenta(),
                        "=>".red()
                    )?;
                    output_stream.flush()?;
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }
                Some(n) if n.trim().trim_end().is_empty() => {
                    write!(
                        output_stream,
                        "{}{}\r",
                        termion::cursor::Up(1),
                        termion::clear::CurrentLine,
                    )?;
                    write!(
                        output_stream,
                        "{} {} {} ",
                        prompt.red(),
                        format!("(requirements: {})", requirements).magenta(),
                        "=>".red()
                    )?;
                    output_stream.flush()?;
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }
                Some(n) => {
                    let trimmed = n.trim().trim_end();
                    let parsed = trimmed
                        .parse()
                        .map_err(|_| Error::ParseError(trimmed.to_string()))?;
                    if validate(&parsed) {
                        Ok::<_, Error>(parsed)
                    } else {
                        write!(output_stream, "\r{}", termion::cursor::Up(1),)?;
                        write!(
                            output_stream,
                            "{} {} {} {}",
                            prompt.red(),
                            format!("(requirements: {})", requirements).magenta(),
                            "=>".red(),
                            n.white()
                        )?;
                        output_stream.flush()?;
                        std::thread::sleep(Duration::from_millis(500));
                        continue;
                    }
                }
            }?;
            return Ok(r);
        }
    }

    /// Show a loading animation using braille until a task completes,
    /// The text is static and never changes.
    ///
    /// The task should return a result, the output will show a ✔️ or ❌
    /// depending on this value.
    ///
    /// # Returns
    /// Returns the result of the given task
    ///
    /// # Usage
    /// ```rust
    /// use std::time::Duration;
    /// use clytia::Clytia;
    ///
    /// let mut cli = Clytia::default();
    ///
    /// cli.static_background_spinner("Waiting for 2 seconds", || -> Result<(), ()> {
    ///     std::thread::sleep(Duration::from_secs(2));
    ///     Ok(())
    /// });
    /// ```
    pub fn static_background_spinner<S, F, R, E>(
        &mut self,
        text: S,
        task: F,
    ) -> Result<std::result::Result<R, E>>
    where
        S: std::fmt::Display + Sync,
        F: Fn() -> std::result::Result<R, E>,
        O: Send,
    {
        let output_stream = &mut self.output;

        let should_stop = AtomicBool::new(false);
        let ret = scope::<_, Result<Result<R, E>>>(|scope| {
            let spinner = scope.spawn::<_, Result<()>>(|_| {
                let mut index = 0;
                while !should_stop.load(std::sync::atomic::Ordering::SeqCst) {
                    write!(
                        output_stream,
                        "\r{} {}",
                        SPINNER_SYMBOLS[index].blue(),
                        text
                    )?;
                    output_stream.flush()?;
                    std::thread::sleep(Duration::from_millis(50));
                    index = (index + 1) % SPINNER_SYMBOLS.len();
                }
                Ok(())
            });

            let ret = task();
            should_stop.store(true, std::sync::atomic::Ordering::SeqCst);
            spinner.join().unwrap()?;

            Ok(ret)
        })
        .unwrap()?;

        match &ret {
            Ok(_) => {
                writeln!(output_stream, "\r{}", format!("✔️  {}", text).green())
            }
            Err(_) => {
                writeln!(output_stream, "\r{}", format!("❌ {}", text).red())
            }
        }?;
        output_stream.flush()?;

        Ok(ret)
    }

    /// Show a loading animating using braille until a task completes,
    /// The text is dynamic and is generated from the `text` parameter.
    ///
    /// The `task` should return a result, which will be used to change the final output of the
    /// spinner with a ✔️ or ❌, depending on success or failure.
    ///
    /// # Usage
    /// ```rust
    /// use std::{time::Duration, sync::atomic::{AtomicUsize, Ordering}};
    /// use clytia::Clytia;
    ///
    /// let mut cli = Clytia::default();
    ///
    /// let counter = AtomicUsize::new(0);
    /// cli.dynamic_background_spinner(
    ///     || {
    ///         let count = counter.load(Ordering::SeqCst);
    ///         if count >= 2_000 {
    ///             "Waited for 2secs".to_string()
    ///         } else {
    ///             format!("{}ms left", 2_000 - count)
    ///         }
    ///     },
    ///     || -> Result<(), ()> {
    ///         let mut count = counter.load(Ordering::SeqCst);
    ///         while count < 2_000 {
    ///             std::thread::sleep(Duration::from_millis(1));
    ///             count = counter.fetch_add(1, Ordering::SeqCst);
    ///         }
    ///         Ok(())
    ///     });
    /// ```
    pub fn dynamic_background_spinner<S, P, F, R, E>(
        &mut self,
        text_func: S,
        task: F,
    ) -> Result<Result<R, E>>
    where
        S: Fn() -> P + Sync,
        P: std::fmt::Display,
        F: Fn() -> Result<R, E>,
        O: Send,
    {
        let output_stream = &mut self.output;

        let should_stop = AtomicBool::new(false);
        let ret = scope::<_, Result<Result<R, E>>>(|scope| {
            let spinner = scope.spawn::<_, Result<()>>(|_| {
                let mut index = 0;
                while !should_stop.load(std::sync::atomic::Ordering::SeqCst) {
                    write!(
                        output_stream,
                        "{}\r{} {}",
                        termion::clear::CurrentLine,
                        SPINNER_SYMBOLS[index].blue(),
                        text_func()
                    )?;
                    index = (index + 1) % SPINNER_SYMBOLS.len();
                    output_stream.flush()?;
                    std::thread::sleep(Duration::from_millis(50));
                }
                Ok(())
            });

            let ret = task();
            should_stop.store(true, std::sync::atomic::Ordering::SeqCst);
            spinner.join().unwrap()?;

            Ok(ret)
        })
        .unwrap()?;

        match &ret {
            Ok(_) => {
                writeln!(
                    output_stream,
                    "{}\r{}",
                    termion::clear::CurrentLine,
                    format!("✔️  {}", text_func()).green()
                )
            }
            Err(_) => {
                writeln!(
                    output_stream,
                    "{}\r{}",
                    termion::clear::CurrentLine,
                    format!("❌ {}", text_func()).red()
                )
            }
        }?;
        output_stream.flush()?;

        Ok(ret)
    }

    /// Run a background task and display a progess bar with a percentage.
    ///
    /// The `progress_func` parameter should return a number between `0` and `100`.
    ///
    /// # Usage
    /// ```rust
    /// use std::{time::Duration, sync::atomic::{AtomicUsize, Ordering}};
    /// use clytia::Clytia;
    ///
    /// let mut cli = Clytia::default();
    ///
    /// let counter = AtomicUsize::new(0);
    /// cli.progress_bar(
    ///     "Wait 2secs",
    ///     || { counter.load(Ordering::SeqCst) / 20 },
    ///     || -> Result<(), ()> {
    ///         let mut count = counter.load(Ordering::SeqCst);
    ///         while count < 2_000 {
    ///             std::thread::sleep(Duration::from_millis(1));
    ///             count = counter.fetch_add(1, Ordering::SeqCst);
    ///         }
    ///         Ok(())
    ///     });
    /// ```
    pub fn progress_bar<S, P, F, R, E>(
        &mut self,
        prompt: S,
        progress_func: P,
        task: F,
    ) -> Result<Result<R, E>>
    where
        S: std::fmt::Display + Sync,
        P: Fn() -> usize + Sync,
        F: Fn() -> Result<R, E>,
        O: Send,
    {
        let output_stream = &mut self.output;

        let should_stop = AtomicBool::new(false);

        let ret = scope::<_, Result<Result<R, E>>>(|scope| {
            scope.spawn::<_, Result<()>>(|_| {
                // Drop the cursor down one line to start with.
                write!(output_stream, "\r")?;
                while !should_stop.load(std::sync::atomic::Ordering::SeqCst) {
                    let mut progress = progress_func();
                    let complete = progress >= 100;
                    if progress > 100 {
                        progress = 100;
                    }

                    // Clear the line, move up, clear that line, go to the start
                    write!(
                        output_stream,
                        "{}{}\r{}{}",
                        termion::clear::CurrentLine,
                        termion::cursor::Up(1),
                        termion::clear::CurrentLine,
                        termion::cursor::Hide
                    )?;
                    writeln!(output_stream, "{}", prompt)?;

                    let cols: usize = termion::terminal_size()?.0.into();
                    let bar_max_len = cols - 9;

                    if !complete {
                        let bar_len =
                            ((bar_max_len as f64 / 100f64) * (progress as f64).round()) as usize;

                        write!(
                            output_stream,
                            "{}",
                            format!(
                                "[{}>{}| {:03}%]",
                                "=".repeat(bar_len),
                                " ".repeat(bar_max_len - bar_len),
                                progress
                            )
                            .blue()
                        )?;
                    } else {
                        write!(
                            output_stream,
                            "{}",
                            format!("[{}=| {:03}%]", "=".repeat(bar_max_len), progress).blue()
                        )?;
                    }
                    output_stream.flush()?;
                    std::thread::sleep(Duration::from_millis(50))
                }

                Ok(())
            });

            let ret = task();
            should_stop.store(true, std::sync::atomic::Ordering::SeqCst);

            Ok(ret)
        })
        .unwrap()?;

        match &ret {
            Ok(_) => {
                writeln!(
                    output_stream,
                    "{}{}\r{}{}✔️  {}",
                    termion::clear::CurrentLine,
                    termion::cursor::Up(1),
                    termion::clear::CurrentLine,
                    termion::cursor::Hide,
                    prompt.green()
                )?;
            }
            Err(_) => {
                let mut progress = progress_func();
                if progress > 100 {
                    progress = 100;
                }

                writeln!(
                    output_stream,
                    "{}{}\r{}{}❌ {}",
                    termion::clear::CurrentLine,
                    termion::cursor::Up(1),
                    termion::clear::CurrentLine,
                    termion::cursor::Hide,
                    prompt.red()
                )?;

                let cols: usize = termion::terminal_size()?.0.into();
                // Weird thing where ❌ is wider is last char goes on newline, so minus one character.
                // (maybe because ❌ is two bytes?)
                let bar_max_len = cols - 10;

                let bar_len = ((bar_max_len as f64 / 100f64) * (progress as f64).round()) as usize;

                writeln!(
                    output_stream,
                    "{}",
                    format!(
                        "[{}❌{}| {:03}%]",
                        "=".repeat(bar_len),
                        " ".repeat(bar_max_len - bar_len),
                        progress
                    )
                    .red()
                )?;
            }
        }

        Ok(ret)
    }

    /// Present several options to the user for them to pick from.
    /// They can use the up and down arrow keys to highlight the option,
    /// and enter to select it.
    ///
    /// # Usage
    /// ```rust
    /// use clytia::Clytia;
    ///
    /// let mut cli = Clytia::default();
    /// let selection = cli.options_menu(vec!["cats", "dogs", "both"]).unwrap();
    ///
    /// println!("You selected: {}", selection);
    /// ```
    pub fn options_menu<S, T>(&mut self, options: S) -> Result<T>
    where
        S: AsRef<[T]>,
        T: std::fmt::Display + Clone,
    {
        let output_stream = &mut self.output;
        let mut output_stream = output_stream.into_raw_mode()?;
        let input_stream = &mut self.input;

        let options_count = options.as_ref().len();
        let mut selected: usize = 0;

        for (index, option) in options.as_ref().iter().enumerate() {
            if index == selected {
                writeln!(
                    output_stream,
                    "{}{}\r",
                    format!("=> {}", option).blue(),
                    termion::cursor::Hide
                )?;
            } else {
                writeln!(output_stream, "   {}{}\r", option, termion::cursor::Hide)?;
            }
        }
        for c in input_stream.keys() {
            match c? {
                Key::Up => selected = (selected + options_count - 1) % options_count,
                Key::Down => selected = (selected + 1) % options_count,
                Key::Char('\n') => break,
                _ => {}
            }

            for _ in 0..options_count {
                write!(
                    output_stream,
                    "{}{}",
                    termion::cursor::Up(1),
                    termion::clear::CurrentLine
                )?;
            }
            write!(output_stream, "\r")?;
            for (index, option) in options.as_ref().iter().enumerate() {
                if index == selected {
                    writeln!(
                        output_stream,
                        "{}{}\r",
                        format!("=> {}", option).blue(),
                        termion::cursor::Hide
                    )?;
                } else {
                    writeln!(output_stream, "   {}{}\r", option, termion::cursor::Hide)?;
                }
            }
        }

        for _ in 0..options_count {
            write!(
                output_stream,
                "{}{}",
                termion::cursor::Up(1),
                termion::clear::CurrentLine
            )?;
        }
        writeln!(
            output_stream,
            "{}",
            format!("\r=> {}\r", options.as_ref()[selected]).green()
        )?;

        Ok(options.as_ref()[selected].clone())
    }

    /// Presents multiple options to the user for them to select,
    /// they can pick multiple. Up and Down arrow keys to change highlighted
    /// option, space to modify selection, enter to confirm choices.
    ///
    /// # Usage
    /// ```rust
    /// use clytia::Clytia;
    ///
    /// let mut cli = Clytia::default();
    ///
    /// let choices = cli.multichoice(vec!["cats", "dogs", "rabbits"]).unwrap();
    ///
    /// println!("You selected: {:?}", choices);
    /// ```
    pub fn multichoice<S, T>(&mut self, options: S) -> Result<Vec<T>>
    where
        S: AsRef<[T]>,
        T: std::fmt::Display + Clone,
    {
        let output_stream = &mut self.output;
        let mut output_stream = output_stream.into_raw_mode()?;
        let input_stream = &mut self.input;

        let mut highlighted: usize = 0;
        let options_count = options.as_ref().len();
        let mut selected = HashSet::new();

        write!(output_stream, "{}", termion::cursor::Hide)?;

        for (index, option) in options.as_ref().iter().enumerate() {
            if selected.contains(&index) {
                if highlighted == index {
                    writeln!(output_stream, "\r{}", format!("[X] {}", option).blue())?;
                } else {
                    writeln!(output_stream, "\r[X] {}", option)?;
                }
            } else if highlighted == index {
                writeln!(output_stream, "\r{}", format!("[ ] {}", option).blue())?;
            } else {
                writeln!(output_stream, "\r[ ] {}", option)?;
            }
        }

        for c in input_stream.keys() {
            match c? {
                Key::Up => highlighted = (highlighted + options_count - 1) % options_count,
                Key::Down => highlighted = (highlighted + 1) % options_count,
                Key::Char(' ') if selected.contains(&highlighted) => {
                    selected.remove(&highlighted);
                }
                Key::Char(' ') => {
                    selected.insert(highlighted);
                }
                Key::Char('\n') => break,
                _ => {}
            }

            for _ in 0..options_count {
                write!(
                    output_stream,
                    "{}{}",
                    termion::cursor::Up(1),
                    termion::clear::CurrentLine
                )?;
            }
            write!(output_stream, "\r")?;
            for (index, option) in options.as_ref().iter().enumerate() {
                if selected.contains(&index) {
                    if highlighted == index {
                        writeln!(output_stream, "\r{}", format!("[X] {}", option).blue())?;
                    } else {
                        writeln!(output_stream, "\r[X] {}", option)?;
                    }
                } else if highlighted == index {
                    writeln!(output_stream, "\r{}", format!("[ ] {}", option).blue())?;
                } else {
                    writeln!(output_stream, "\r[ ] {}", option)?;
                }
            }
        }

        for _ in 0..options_count {
            write!(
                output_stream,
                "{}{}",
                termion::cursor::Up(1),
                termion::clear::CurrentLine
            )?;
        }
        write!(output_stream, "\r")?;

        let returns = options
            .as_ref()
            .iter()
            .enumerate()
            .filter(|(index, _)| selected.contains(index))
            .map(|(_, option)| option.clone())
            .collect();

        for option in &returns {
            writeln!(output_stream, "{}", format!("\r[X] {}\r", option).green())?;
        }

        Ok(returns)
    }
}

impl Default for Clytia<Stdin, Stdout> {
    fn default() -> Self {
        Self {
            input: io::stdin(),
            output: io::stdout(),
        }
    }
}

#[cfg(test)]
mod tests {
    mod non_interactive {
        use std::time::Duration;

        use owo_colors::OwoColorize;

        use crate::{Clytia, SPINNER_SYMBOLS};

        #[test]
        fn test_parsed_input_with_default() {
            let output = Vec::new();
            let input: Vec<u8> = vec![b'1', b'\n'];
            let mut cli = Clytia::new(&input as &[u8], output);
            assert!(cli.parsed_input("input a number", Some(0)).is_ok());
            let s = std::str::from_utf8(cli.output());
            assert!(s.is_ok());
            let s = s.unwrap();
            assert_eq!(
                s,
                format!(
                    "{}{}{}",
                    "input a number".blue(),
                    "(default: 0)".magenta(),
                    " => ".blue()
                )
            )
        }

        #[test]
        fn test_parsed_input_without_default() {
            let output = Vec::new();
            let input: Vec<u8> = vec![b'1', b'\n'];
            let mut cli = Clytia::new(&input as &[u8], output);
            assert!(cli.parsed_input::<_, usize>("input a number", None).is_ok());
            let s = std::str::from_utf8(cli.output());
            assert!(s.is_ok());
            let s = s.unwrap();
            assert_eq!(s, "input a number => ".blue().to_string())
        }

        #[test]
        fn test_static_spinner_success() {
            let output = Vec::new();
            let input: Vec<u8> = Vec::new();
            let mut cli = Clytia::new(&input as &[u8], output);
            assert!(cli
                .static_background_spinner::<_, _, (), ()>("Wait 100ms", || {
                    std::thread::sleep(Duration::from_millis(100));
                    Ok(())
                })
                .is_ok());
            let s = std::str::from_utf8(cli.output());
            assert!(s.is_ok());
            let s = s.unwrap();
            assert_eq!(
                s,
                format!(
                    "\r{} Wait 100ms\r{} Wait 100ms\r{}\n",
                    SPINNER_SYMBOLS[0].blue(),
                    SPINNER_SYMBOLS[1].blue(),
                    "✔️  Wait 100ms".green()
                )
            );
        }

        #[test]
        fn test_static_spinner_failure() {
            let output = Vec::new();
            let input: Vec<u8> = Vec::new();
            let mut cli = Clytia::new(&input as &[u8], output);
            assert!(cli
                .static_background_spinner::<_, _, (), ()>("Wait 100ms", || {
                    std::thread::sleep(Duration::from_millis(100));
                    Err(())
                })
                .is_ok());
            let s = std::str::from_utf8(cli.output());
            assert!(s.is_ok());
            let s = s.unwrap();
            assert_eq!(
                s,
                format!(
                    "\r{} Wait 100ms\r{} Wait 100ms\r{}\n",
                    SPINNER_SYMBOLS[0].blue(),
                    SPINNER_SYMBOLS[1].blue(),
                    "❌ Wait 100ms".red()
                )
            );
        }

        #[test]
        fn test_dynamic_spinner_success() {
            let output = Vec::new();
            let input: Vec<u8> = Vec::new();
            let mut cli = Clytia::new(&input as &[u8], output);
            assert!(cli
                .dynamic_background_spinner::<_, _, _, (), ()>(
                    || "Wait 100ms",
                    || {
                        std::thread::sleep(Duration::from_millis(100));
                        Ok(())
                    }
                )
                .is_ok());
            let s = std::str::from_utf8(cli.output());
            assert!(s.is_ok());
            let s = s.unwrap();
            assert_eq!(
                s,
                format!(
                    "{}\r{} Wait 100ms{}\r{} Wait 100ms{}\r{}\n",
                    termion::clear::CurrentLine,
                    SPINNER_SYMBOLS[0].blue(),
                    termion::clear::CurrentLine,
                    SPINNER_SYMBOLS[1].blue(),
                    termion::clear::CurrentLine,
                    "✔️  Wait 100ms".green()
                )
            );
        }

        #[test]
        fn test_dynamic_spinner_failure() {
            let output = Vec::new();
            let input: Vec<u8> = Vec::new();
            let mut cli = Clytia::new(&input as &[u8], output);
            assert!(cli
                .dynamic_background_spinner::<_, _, _, (), ()>(
                    || "Wait 100ms",
                    || {
                        std::thread::sleep(Duration::from_millis(100));
                        Err(())
                    }
                )
                .is_ok());
            let s = std::str::from_utf8(cli.output());
            assert!(s.is_ok());
            let s = s.unwrap();
            assert_eq!(
                s,
                format!(
                    "{}\r{} Wait 100ms{}\r{} Wait 100ms{}\r{}\n",
                    termion::clear::CurrentLine,
                    SPINNER_SYMBOLS[0].blue(),
                    termion::clear::CurrentLine,
                    SPINNER_SYMBOLS[1].blue(),
                    termion::clear::CurrentLine,
                    "❌ Wait 100ms".red()
                )
            );
        }
    }

    mod interactive {
        use std::{sync::atomic::AtomicUsize, time::Duration};

        use crate::Clytia;

        #[test]
        fn test_progress_bar_success() {
            let output = std::io::stdout();
            let input = std::io::stdin();
            let counter = AtomicUsize::new(0);
            let mut cli = Clytia::new(input, output);
            assert!(cli
                .progress_bar::<_, _, _, (), ()>(
                    "Wait 10000ms",
                    || { counter.load(std::sync::atomic::Ordering::SeqCst) / 100 },
                    || {
                        while counter.load(std::sync::atomic::Ordering::SeqCst) <= 10_000 {
                            std::thread::sleep(Duration::from_millis(1));
                            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        }
                        Ok(())
                    }
                )
                .is_ok());
        }

        #[test]
        fn test_progress_bar_failure() {
            let output = std::io::stdout();
            let input = std::io::stdin();
            let counter = AtomicUsize::new(0);
            let mut cli = Clytia::new(input, output);
            assert!(cli
                .progress_bar::<_, _, _, (), ()>(
                    "Wait 10000ms",
                    || { counter.load(std::sync::atomic::Ordering::SeqCst) / 100 },
                    || {
                        while counter.load(std::sync::atomic::Ordering::SeqCst) <= 5_000 {
                            std::thread::sleep(Duration::from_millis(1));
                            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        }
                        Err(())
                    }
                )
                .is_ok());
        }

        #[test]
        fn test_options_menu() {
            let output = std::io::stdout();
            let input = std::io::stdin();

            let mut cli = Clytia::new(input, output);
            assert!(cli.options_menu(vec!["cats", "dogs", "both"]).is_ok())
        }

        #[test]
        fn test_multichoice() {
            let output = std::io::stdout();
            let input = std::io::stdin();

            let mut cli = Clytia::new(input, output);
            assert!(cli.multichoice(vec!["cats", "dogs", "rabbits"]).is_ok())
        }
    }
}
