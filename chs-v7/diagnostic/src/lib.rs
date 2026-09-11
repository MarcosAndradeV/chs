/// The default Result type of chs compiler.
pub type ChsResult<T> = anyhow::Result<T>;
pub use anyhow::bail;

use lex_just_parse::lexer::Loc;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub loc: Loc,
    pub message: String,
}

pub struct DiagnosticReporter {
    pub errors: Vec<Diagnostic>,
}

impl Default for DiagnosticReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosticReporter {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    pub fn report(&mut self, loc: Loc, message: impl Into<String>) {
        self.errors.push(Diagnostic {
            loc,
            message: message.into(),
        });
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn print_all(&self) {
        for err in &self.errors {
            println!("Error at {}: {}", err.loc, err.message);
        }
    }

    pub fn into_string(&self) -> String {
        use std::fmt::Write as _;
        let mut buffer = String::new();
        for err in &self.errors {
            _ = writeln!(&mut buffer, "Error at {}: {}", err.loc, err.message);
        }
        buffer
    }
}
