use std::env;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::process::ExitCode;

use open_sdbl::{Diagnostic, tokenize};

const HELP: &str = "open-sdbl — tooling for the 1C query language\n\n\
Usage:\n  open-sdbl lex [FILE|-]\n  open-sdbl --help\n\n\
Commands:\n  lex    Print lexical tokens; reads standard input when FILE is '-' or omitted\n";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}

fn run() -> Result<(), CliError> {
    let mut arguments = env::args().skip(1);
    let Some(command) = arguments.next() else {
        print!("{HELP}");
        return Ok(());
    };

    match command.as_str() {
        "-h" | "--help" => {
            print!("{HELP}");
            Ok(())
        }
        "lex" => {
            let path = arguments.next().unwrap_or_else(|| "-".to_owned());
            if let Some(unexpected) = arguments.next() {
                return Err(CliError::Usage(format!(
                    "unexpected argument {unexpected:?}\n\n{HELP}"
                )));
            }
            lex(&path)
        }
        unknown => Err(CliError::Usage(format!(
            "unknown command {unknown:?}\n\n{HELP}"
        ))),
    }
}

fn lex(path: &str) -> Result<(), CliError> {
    let source = if path == "-" {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|error| CliError::Io("cannot read standard input".to_owned(), error))?;
        source
    } else {
        fs::read_to_string(path)
            .map_err(|error| CliError::Io(format!("cannot read {path:?}"), error))?
    };

    for token in tokenize(&source).map_err(CliError::Lexical)? {
        println!(
            "{}:{}\t{}\t{}",
            token.span.line,
            token.span.column,
            token.kind,
            escape_lexeme(token.lexeme)
        );
    }
    Ok(())
}

fn escape_lexeme(lexeme: &str) -> String {
    let mut escaped = String::with_capacity(lexeme.len());
    for character in lexeme.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Io(String, io::Error),
    Lexical(Diagnostic),
}

impl CliError {
    const fn exit_code(&self) -> u8 {
        match self {
            Self::Lexical(_) => 1,
            Self::Usage(_) | Self::Io(_, _) => 2,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) => formatter.write_str(message),
            Self::Io(context, error) => write!(formatter, "{context}: {error}"),
            Self::Lexical(error) => error.fmt(formatter),
        }
    }
}
