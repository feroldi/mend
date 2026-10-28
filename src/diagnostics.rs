use std::fmt;

use crate::source_map::{SourceMap, Span};

#[derive(Default)]
pub(crate) struct DiagnosticBag {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticBag {
    pub(crate) fn new() -> DiagnosticBag {
        Default::default()
    }

    pub(crate) fn with_diag(diagnostic: Diagnostic) -> DiagnosticBag {
        DiagnosticBag {
            diagnostics: vec![diagnostic],
        }
    }

    pub(crate) fn add(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub(crate) fn merge(&mut self, other: DiagnosticBag) {
        self.diagnostics.extend(other.diagnostics);
    }

    pub(crate) fn render(&self, source_map: &SourceMap) -> String {
        self.diagnostics
            .iter()
            .map(|diagnostic| diagnostic.render(source_map))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub(crate) struct Diagnostic {
    pub(crate) code: DiagnosticCode,
    pub(crate) message: String,
    pub(crate) label: Option<Label>,
    pub(crate) helps: Vec<String>,
}

pub(crate) struct Label {
    pub(crate) span: Span,
    pub(crate) message: String,
}

#[derive(Clone, Copy)]
pub(crate) enum DiagnosticCode {
    E0001,
    E0002,
    E0003,
    E0004,
}

impl Diagnostic {
    fn render(&self, source_map: &SourceMap) -> String {
        let mut rendered = format!("error[{}]: {}\n", self.code, self.message);

        let gutter = match &self.label {
            Some(label) => {
                let (line, column) = source_map.lookup_line_and_column(label.span.start);
                let gutter = " ".repeat(line.to_string().len());
                let caret_count = source_map.get_text_snippet(label.span).chars().count();

                rendered += &format!("{gutter}--> {}:{line}:{column}\n", source_map.get_name());
                rendered += &format!("{gutter} |\n");
                rendered += &format!("{line} | {}\n", source_map.get_line_text(label.span.start));
                rendered += &format!(
                    "{gutter} | {}{} {}\n",
                    " ".repeat(column - 1),
                    "^".repeat(caret_count.max(1)),
                    label.message
                );

                gutter
            }
            None => " ".into(),
        };

        for help in &self.helps {
            rendered += &format!("{gutter} = help: {help}\n");
        }

        rendered
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticCode::E0001 => write!(f, "E0001"),
            DiagnosticCode::E0002 => write!(f, "E0002"),
            DiagnosticCode::E0003 => write!(f, "E0003"),
            DiagnosticCode::E0004 => write!(f, "E0004"),
        }
    }
}
