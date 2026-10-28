pub(crate) struct SourceMap {
    source_name: String,
    source_text: String,
}

impl SourceMap {
    pub(crate) const DEFAULT_SOURCE_NAME: &'static str = "<source>";

    pub(crate) fn new(source_name: String, source_text: String) -> SourceMap {
        SourceMap {
            source_name,
            source_text,
        }
    }

    pub(crate) fn get_name(&self) -> &str {
        &self.source_name
    }

    pub(crate) fn get_text(&self) -> &str {
        &self.source_text
    }

    pub(crate) fn get_text_snippet(&self, span: Span) -> &str {
        &self.source_text[span.start.0..span.end.0]
    }

    /// Both the line and the column start at 1. The column counts chars, not bytes.
    pub(crate) fn lookup_line_and_column(&self, pos: BytePos) -> (usize, usize) {
        let text_before_pos = &self.source_text[..pos.0];
        let line = text_before_pos.matches('\n').count() + 1;
        let column = text_before_pos[self.find_line_start(pos)..].chars().count() + 1;

        (line, column)
    }

    pub(crate) fn get_line_text(&self, pos: BytePos) -> &str {
        let line_end = self.source_text[pos.0..]
            .find('\n')
            .map_or(self.source_text.len(), |idx| pos.0 + idx);

        &self.source_text[self.find_line_start(pos)..line_end]
    }

    fn find_line_start(&self, pos: BytePos) -> usize {
        self.source_text[..pos.0]
            .rfind('\n')
            .map_or(0, |idx| idx + 1)
    }
}

/// This is an exclusive text range as in [start, end).
#[derive(Clone, Copy)]
pub(crate) struct Span {
    pub(crate) start: BytePos,
    pub(crate) end: BytePos,
}

#[derive(Clone, Copy)]
pub(crate) struct BytePos(pub(crate) usize);
