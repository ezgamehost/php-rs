/// Value operations can report into either the compiler's unfiltered diagnostics
/// or a request's live error-reporting mask.
pub(crate) trait WarningSink {
    fn push(&mut self, message: String);
}

impl WarningSink for Vec<String> {
    fn push(&mut self, message: String) {
        Vec::push(self, message);
    }
}

pub(crate) const FATAL_ERRORS: i64 = 1 | 4 | 16 | 64 | 256 | 4096;

pub(crate) struct Diagnostics {
    pub messages: Vec<String>,
    pub reporting: i64,
    pub compiling: bool,
    pub source: std::rc::Rc<crate::source::Source>,
    pub output_position: usize,
    pub events: Vec<Diagnostic>,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            reporting: 30719,
            compiling: false,
            source: Default::default(),
            output_position: 0,
            events: Vec::new(),
        }
    }
}

impl Diagnostics {
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
    pub fn extend(&mut self, messages: impl IntoIterator<Item = String>) {
        for message in messages {
            self.push(message);
        }
    }
}

impl WarningSink for Diagnostics {
    fn push(&mut self, message: String) {
        let level = if message.starts_with("Deprecated:") {
            8192
        } else if message.starts_with("Notice:") {
            8
        } else if self.compiling {
            128
        } else {
            2
        };
        if self.reporting & level != 0 {
            let (label, text) = if let Some(text) = message.strip_prefix("Deprecated: ") {
                ("Deprecated", text)
            } else if let Some(text) = message.strip_prefix("Notice: ") {
                ("Notice", text)
            } else {
                (
                    "Warning",
                    message.strip_prefix("Warning: ").unwrap_or(&message),
                )
            };
            let (text, line) = text
                .rsplit_once(" on line ")
                .and_then(|(text, line)| line.parse::<usize>().ok().map(|line| (text, line)))
                .unwrap_or((text, 0));
            let mut rendered = format!("{label}: {text} in ").into_bytes();
            rendered.extend_from_slice(&self.source.name);
            rendered.extend_from_slice(format!(" on line {line}\n").as_bytes());
            self.events.push(Diagnostic {
                position: self.output_position,
                rendered,
            });
            self.messages.push(message);
        }
    }
}

/// A displayed diagnostic at its position in the request's ordinary byte output.
pub(crate) struct Diagnostic {
    pub position: usize,
    pub rendered: Vec<u8>,
}
