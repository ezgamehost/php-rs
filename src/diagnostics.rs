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
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            reporting: 30719,
            compiling: false,
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
            self.messages.push(message);
        }
    }
}
