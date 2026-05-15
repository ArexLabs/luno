#[derive(Debug, Clone)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub col: usize,
}

impl Span {
    pub fn new(start: usize, end: usize, line: usize, col: usize) -> Self {
        Span { start, end, line, col }
    }

    pub fn dummy() -> Self {
        Span { start: 0, end: 0, line: 0, col: 0 }
    }

    pub fn merge(&self, other: &Span) -> Span {
        let s = self.start.min(other.start);
        let e = self.end.max(other.end);
        Span { start: s, end: e, line: self.line, col: self.col }
    }
}

#[derive(Debug, Clone)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone)]
pub struct Diag {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub notes: Vec<String>,
}

impl Diag {
    pub fn error(msg: impl Into<String>, span: Span) -> Self {
        Diag { severity: Severity::Error, message: msg.into(), span, notes: vec![] }
    }

    pub fn warning(msg: impl Into<String>, span: Span) -> Self {
        Diag { severity: Severity::Warning, message: msg.into(), span, notes: vec![] }
    }

    pub fn note(mut self, msg: impl Into<String>) -> Self {
        self.notes.push(msg.into());
        self
    }
}

#[derive(Debug, Default)]
pub struct Diagnostics {
    pub diags: Vec<Diag>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Diagnostics { diags: vec![] }
    }

    pub fn push(&mut self, diag: Diag) {
        self.diags.push(diag);
    }

    pub fn extend(&mut self, other: &mut Diagnostics) {
        self.diags.append(&mut other.diags);
    }

    pub fn has_errors(&self) -> bool {
        self.diags.iter().any(|d| matches!(d.severity, Severity::Error))
    }

    pub fn emit(&self, source: &str) {
        let lines: Vec<&str> = source.lines().collect();
        for diag in &self.diags {
            match diag.severity {
                Severity::Error => eprint!("  error"),
                Severity::Warning => eprint!("warning"),
                Severity::Note => eprint!("  note"),
            }
            if diag.span.line > 0 {
                eprint!(":{}:{}", diag.span.line, diag.span.col);
            }
            eprintln!(": {}", diag.message);
            if diag.span.line > 0 && diag.span.line <= lines.len() {
                let line = lines[diag.span.line - 1];
                eprintln!(" {:>4} | {}", diag.span.line, line);
                let indent = format!(" {:>4} | ", diag.span.line);
                let padding: String = (0..diag.span.col.saturating_sub(1))
                    .map(|_| ' ')
                    .collect();
                let width = (diag.span.end - diag.span.start).max(1);
                let underline: String = (0..width.min(line.len().saturating_sub(diag.span.col.saturating_sub(1))))
                    .map(|_| '^')
                    .collect();
                eprintln!("{}{}", indent, padding);
                eprintln!("{}{}", indent, underline);
            }
            for note in &diag.notes {
                eprintln!("  = note: {}", note);
            }
        }
    }
}


