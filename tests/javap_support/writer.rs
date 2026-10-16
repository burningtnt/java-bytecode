//! A close port of `com.sun.tools.javap.BasicWriter.LineWriter`.
//!
//! javap's writer is line-buffered and defers spaces: `print(" ")` only records
//! a pending space, which is flushed before the next visible character (or
//! discarded at end of line). This, plus `tab()` and indentation, is what makes
//! the column alignment reproducible. We must reproduce it exactly, so this is
//! a line-for-line port rather than something idiomatic.

/// A line-buffered text writer that reproduces javap's spacing behaviour.
#[derive(Debug)]
pub struct Writer {
    out: String,
    buffer: String,
    pending_spaces: usize,
    pending_newline: bool,
    indent_count: i64,
    indent_width: i64,
    tab_column: i64,
}

impl Writer {
    pub fn new() -> Self {
        Self {
            out: String::new(),
            buffer: String::new(),
            pending_spaces: 0,
            pending_newline: false,
            indent_count: 0,
            indent_width: 2,
            tab_column: 40,
        }
    }

    /// Java's `StringBuilder.length()`, i.e. UTF-16 code-unit count.
    fn buffer_len_utf16(&self) -> usize {
        self.buffer.chars().map(char::len_utf16).sum()
    }

    fn indent(&mut self) {
        let spaces = (self.indent_count * self.indent_width).max(0) as usize;
        self.pending_spaces += spaces;
    }

    pub fn print(&mut self, s: &str) {
        if self.pending_newline {
            self.println();
            self.pending_newline = false;
        }
        for c in s.chars() {
            match c {
                ' ' => self.pending_spaces += 1,
                '\n' => self.println(),
                _ => {
                    if self.buffer.is_empty() {
                        self.indent();
                    }
                    for _ in 0..self.pending_spaces {
                        self.buffer.push(' ');
                    }
                    self.pending_spaces = 0;
                    self.buffer.push(c);
                }
            }
        }
    }

    pub fn println(&mut self) {
        self.pending_spaces = 0;
        self.out.push_str(&self.buffer);
        self.out.push('\n');
        self.buffer.clear();
    }

    pub fn println_str(&mut self, s: &str) {
        self.print(s);
        self.println();
    }

    pub fn indent_by(&mut self, delta: i64) {
        self.indent_count += delta;
    }

    pub fn tab(&mut self) {
        let col = self.indent_count * self.indent_width + self.tab_column;
        let buffer_len = self.buffer_len_utf16() as i64;
        self.pending_spaces += if col <= buffer_len {
            1
        } else {
            (col - buffer_len) as usize
        };
    }

    pub fn set_pending_newline(&mut self, value: bool) {
        self.pending_newline = value;
    }

    /// Writes an `Error: …` line directly, bypassing the line buffer (matching
    /// `BasicWriter.report`, whose `PrintWriter` is not the `LineWriter`).
    pub fn report(&mut self, message: &str) {
        self.out.push_str("Error: ");
        self.out.push_str(message);
        self.out.push('\n');
    }

    pub fn finish(self) -> String {
        self.out
    }
}

impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}
