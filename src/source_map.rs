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
}

/// This is an exclusive text range as in [start, end).
#[derive(Clone, Copy)]
pub(crate) struct Span {
    pub(crate) start: BytePos,
    pub(crate) end: BytePos,
}

#[derive(Clone, Copy)]
pub(crate) struct BytePos(pub(crate) usize);
