// Write Markdown to the terminal

use crate::term2::{color, Attr, Terminal};
use markdown::tokenize;
use markdown::{Block, ListItem, Span};
use std::io;

// Handles the wrapping of text written to the console
struct LineWrapper<'a, T: Terminal> {
    indent: u32,
    margin: u32,
    pos: u32,
    pub w: &'a mut T,
}

impl<'a, T: Terminal + 'a> LineWrapper<'a, T> {
    // Just write a newline
    fn write_line(&mut self) {
        let _ = writeln!(self.w);
        // Reset column position to start of line
        self.pos = 0;
    }
    // Called before writing text to ensure indent is applied
    fn write_indent(&mut self) {
        if self.pos == 0 {
            // Write a space for each level of indent
            for _ in 0..self.indent {
                let _ = write!(self.w, " ");
            }
            self.pos = self.indent;
        }
    }
    // Write a non-breaking word
    fn write_word(&mut self, word: &str) {
        // Ensure correct indentation
        self.write_indent();
        let word_len = word.len() as u32;

        // If this word goes past the margin
        if self.pos + word_len > self.margin {
            // And adding a newline would give us more space
            if self.pos > self.indent {
                // Then add a newline!
                self.write_line();
                self.write_indent();
            }
        }

        // Write the word
        let _ = write!(self.w, "{}", word);
        self.pos += word_len;
    }
    fn write_space(&mut self) {
        if self.pos > self.indent {
            if self.pos < self.margin {
                self.write_word(" ");
            } else {
                self.write_line();
            }
        }
    }
    // Writes a span of text which wraps at the margin
    fn write_span(&mut self, text: &str) {
        // Allow words to wrap on whitespace
        let mut is_first = true;
        for word in text.split(char::is_whitespace) {
            if is_first {
                is_first = false;
            } else {
                self.write_space();
            }
            self.write_word(word);
        }
    }
    // Constructor
    fn new(w: &'a mut T, indent: u32, margin: u32) -> Self {
        LineWrapper {
            indent,
            margin,
            pos: indent,
            w,
        }
    }
}

// Handles the formatting of text
struct LineFormatter<'a, T: Terminal + io::Write> {
    wrapper: LineWrapper<'a, T>,
    attrs: Vec<Attr>,
}

impl<'a, T: Terminal + io::Write + 'a> LineFormatter<'a, T> {
    fn new(w: &'a mut T, indent: u32, margin: u32) -> Self {
        LineFormatter {
            wrapper: LineWrapper::new(w, indent, margin),
            attrs: Vec::new(),
        }
    }
    fn push_attr(&mut self, attr: Attr) {
        self.attrs.push(attr);
        let _ = self.wrapper.w.attr(attr);
    }
    fn pop_attr(&mut self) {
        self.attrs.pop();
        let _ = self.wrapper.w.reset();
        for attr in &self.attrs {
            let _ = self.wrapper.w.attr(*attr);
        }
    }
    fn do_spans(&mut self, spans: Vec<Span>) {
        for span in spans {
            match span {
                Span::Break => {}
                Span::Text(text) => {
                    self.wrapper.write_span(&text);
                }
                Span::Code(code) => {
                    self.push_attr(Attr::Bold);
                    self.wrapper.write_word(&code);
                    self.pop_attr();
                }
                Span::Emphasis(spans) => {
                    self.push_attr(Attr::ForegroundColor(color::BRIGHT_RED));
                    self.do_spans(spans);
                    self.pop_attr();
                }
                _ => {}
            }
        }
    }
    fn do_block(&mut self, b: Block) {
        match b {
            Block::Header(spans, _) => {
                self.push_attr(Attr::Bold);
                self.wrapper.write_line();
                self.do_spans(spans);
                self.wrapper.write_line();
                self.pop_attr();
            }
            Block::CodeBlock(code) => {
                self.wrapper.write_line();
                self.wrapper.indent += 2;
                for line in code.lines() {
                    // Don't word-wrap code lines
                    self.wrapper.write_word(line);
                    self.wrapper.write_line();
                }
                self.wrapper.indent -= 2;
            }
            Block::Paragraph(spans) => {
                self.wrapper.write_line();
                self.do_spans(spans);
                self.wrapper.write_line();
            }
            Block::UnorderedList(items) => {
                self.wrapper.write_line();
                for item in items {
                    self.wrapper.indent += 2;
                    match item {
                        ListItem::Simple(spans) => {
                            self.do_spans(spans);
                        }
                        ListItem::Paragraph(blocks) => {
                            for block in blocks {
                                self.do_block(block);
                            }
                        }
                    }
                    self.wrapper.write_line();
                    self.wrapper.indent -= 2;
                }
            }
            _ => {}
        }
    }
}

pub fn md<'a, S: AsRef<str>, T: Terminal + io::Write + 'a>(t: &'a mut T, content: S) {
    let mut f = LineFormatter::new(t, 0, 79);
    let blocks = tokenize(content.as_ref());
    for b in blocks {
        f.do_block(b);
    }
}
