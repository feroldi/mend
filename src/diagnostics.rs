use crate::compiler_context::CompilerContext;
use crate::interner::Symbol;
use crate::scanner::TokenKind;
use crate::source_map::Span;

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

    pub(crate) fn emit(&self, emitter: &mut impl DiagnosticEmitter) {
        for diagnostic in &self.diagnostics {
            emitter.emit(diagnostic);
        }
    }
}

impl From<Diagnostic> for DiagnosticBag {
    fn from(diagnostic: Diagnostic) -> DiagnosticBag {
        DiagnosticBag::with_diag(diagnostic)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Diag {
    MainNotFound,
    MainDeclaredTwice,
    BindingNotFound {
        ident: Symbol,
    },
    AssignToImmutableBinding {
        ident: Symbol,
    },
    ExpectedButFound {
        expected: TokenKind,
        found: TokenKind,
    },
    ExpectedExpression {
        found: TokenKind,
    },
    UnknownCharacter(char),
    InvalidIntegerSuffix {
        suffix: Symbol,
    },
}

impl Diag {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Diag::MainNotFound => "E0001",
            Diag::ExpectedButFound { .. } => "E0002",
            Diag::ExpectedExpression { .. } => "E0003",
            Diag::UnknownCharacter(_) => "E0004",
            Diag::MainDeclaredTwice => "E0005",
            Diag::BindingNotFound { .. } => "E0006",
            Diag::AssignToImmutableBinding { .. } => "E0007",
            Diag::InvalidIntegerSuffix { .. } => "E0008",
        }
    }

    pub(crate) fn message(&self, ctx: &CompilerContext) -> String {
        match self {
            Diag::MainNotFound => "`main` function not found".into(),
            Diag::MainDeclaredTwice => "`main` is declared more than once".into(),
            Diag::BindingNotFound { ident } => format!(
                "cannot find binding `{}` in this scope",
                ctx.resolve_symbol(*ident)
            ),
            Diag::AssignToImmutableBinding { ident } => format!(
                "cannot assign to immutable binding `{}`",
                ctx.resolve_symbol(*ident)
            ),
            Diag::ExpectedButFound { expected, found } => {
                format!("expected {expected}, found {found}")
            }
            Diag::ExpectedExpression { found } => format!("expected expression, found {found}"),
            Diag::UnknownCharacter(ch) => format!("unknown character `{ch}`"),
            Diag::InvalidIntegerSuffix { suffix } => format!(
                "invalid suffix `{}` for integer literal",
                ctx.resolve_symbol(*suffix)
            ),
        }
    }
}

pub(crate) struct Diagnostic {
    pub(crate) diag: Diag,
    pub(crate) label: Option<Label>,
    pub(crate) helps: Vec<String>,
}

impl Diagnostic {
    pub(crate) fn builder(diag: Diag) -> DiagnosticBuilder {
        DiagnosticBuilder {
            diagnostic: Diagnostic {
                diag,
                label: None,
                helps: vec![],
            },
        }
    }
}

pub(crate) struct Label {
    pub(crate) span: Span,
    pub(crate) message: String,
}

pub(crate) struct DiagnosticBuilder {
    diagnostic: Diagnostic,
}

impl DiagnosticBuilder {
    pub(crate) fn label(mut self, span: Span, message: impl Into<String>) -> DiagnosticBuilder {
        self.diagnostic.label = Some(Label {
            span,
            message: message.into(),
        });

        self
    }

    pub(crate) fn help(mut self, message: impl Into<String>) -> DiagnosticBuilder {
        self.diagnostic.helps.push(message.into());

        self
    }

    pub(crate) fn build(self) -> Diagnostic {
        self.diagnostic
    }
}

pub(crate) trait DiagnosticEmitter {
    fn emit(&mut self, diagnostic: &Diagnostic);
}

/// Formats diagnostics for the end user, in the rustc layout.
pub(crate) struct HumanEmitter<'a> {
    ctx: &'a CompilerContext,
    output: String,
}

impl<'a> HumanEmitter<'a> {
    pub(crate) fn new(ctx: &'a CompilerContext) -> HumanEmitter<'a> {
        HumanEmitter {
            ctx,
            output: String::new(),
        }
    }

    pub(crate) fn into_output(self) -> String {
        self.output
    }
}

impl DiagnosticEmitter for HumanEmitter<'_> {
    fn emit(&mut self, diagnostic: &Diagnostic) {
        let source_map = self.ctx.get_source_map();

        if !self.output.is_empty() {
            self.output += "\n";
        }

        self.output += &format!(
            "error[{}]: {}\n",
            diagnostic.diag.code(),
            diagnostic.diag.message(self.ctx)
        );

        let gutter = match &diagnostic.label {
            Some(label) => {
                let (line, column) = source_map.lookup_line_and_column(label.span.start);
                let gutter = " ".repeat(line.to_string().len());
                let caret_count = source_map.get_text_snippet(label.span).chars().count();

                self.output += &format!("{gutter}--> {}:{line}:{column}\n", source_map.get_name());
                self.output += &format!("{gutter} |\n");
                self.output +=
                    &format!("{line} | {}\n", source_map.get_line_text(label.span.start));
                self.output += &format!(
                    "{gutter} | {}{} {}\n",
                    " ".repeat(column - 1),
                    "^".repeat(caret_count.max(1)),
                    label.message
                );

                gutter
            }
            None => " ".into(),
        };

        for help in &diagnostic.helps {
            self.output += &format!("{gutter} = help: {help}\n");
        }
    }
}
