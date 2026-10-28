use std::fmt;

pub(crate) struct Diagnostic {
    pub(crate) code: DiagnosticCode,
    pub(crate) message: String,
    pub(crate) helps: Vec<String>,
}

#[derive(Clone, Copy)]
pub(crate) enum DiagnosticCode {
    E0001,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "error[{}]: {}", self.code, self.message)?;

        for help in &self.helps {
            writeln!(f, "  = help: {}", help)?;
        }

        Ok(())
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticCode::E0001 => write!(f, "E0001"),
        }
    }
}
