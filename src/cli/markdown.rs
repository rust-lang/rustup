// Write Markdown to the terminal

use crate::term2::{color, Attr, Terminal};
use std::io;

use pulldown_cmark::{Event, Tag};

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
    is_code_block: bool,
    wrapper: LineWrapper<'a, T>,
    attrs: Vec<Attr>,
}

impl<'a, T: Terminal + io::Write + 'a> LineFormatter<'a, T> {
    fn new(w: &'a mut T, indent: u32, margin: u32) -> Self {
        LineFormatter {
            is_code_block: false,
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

    fn start_tag(&mut self, tag: Tag<'a>) {
        match tag {
            Tag::Paragraph => {
                self.wrapper.write_line();
            }
            Tag::Heading(_level) => {
                self.push_attr(Attr::Bold);
                self.wrapper.write_line();
            }
            Tag::Table(_alignments) => {}
            Tag::TableHead => {}
            Tag::TableRow => {}
            Tag::TableCell => {}
            Tag::BlockQuote => {}
            Tag::CodeBlock(_lang) => {
                self.wrapper.write_line();
                self.wrapper.indent += 2;
                self.is_code_block = true;
            }
            Tag::List(_) => {
                self.wrapper.write_line();
                self.wrapper.indent += 2;
            }
            Tag::Item => {
                self.wrapper.write_line();
            }
            Tag::Emphasis => {
                self.push_attr(Attr::ForegroundColor(color::BRIGHT_RED));
            }
            Tag::Strong => {}
            Tag::Strikethrough => {}
            Tag::Link(_link_type, _dest, _title) => {}
            Tag::Image(_link_type, _dest, _title) => {}
            Tag::FootnoteDefinition(_name) => {}
        }
    }

    fn end_tag(&mut self, tag: Tag<'a>) {
        match tag {
            Tag::Paragraph => {
                self.wrapper.write_line();
            }
            Tag::Heading(_level) => {
                self.wrapper.write_line();
                self.pop_attr();
            }
            Tag::Table(_) => {}
            Tag::TableHead => {}
            Tag::TableRow => {}
            Tag::TableCell => {}
            Tag::BlockQuote => {}
            Tag::CodeBlock(_) => {
                self.is_code_block = false;
                self.wrapper.indent -= 2;
            }
            Tag::List(_) => {
                self.wrapper.indent -= 2;
                self.wrapper.write_line();
            }
            Tag::Item => {}
            Tag::Emphasis => {
                self.pop_attr();
            }
            Tag::Strong => {}
            Tag::Strikethrough => {}
            Tag::Link(_, _, _) => {}
            Tag::Image(_, _, _) => {} // shouldn't happen, handled in start
            Tag::FootnoteDefinition(_) => {}
        }
    }

    fn process_event(&mut self, event: Event<'a>) {
        use self::Event::*;
        match event {
            Start(tag) => self.start_tag(tag),
            End(tag) => self.end_tag(tag),
            Text(text) => {
                if self.is_code_block {
                    self.wrapper.write_word(&text);
                } else {
                    self.wrapper.write_span(&text);
                }
            }
            Code(code) => {
                self.push_attr(Attr::Bold);
                self.wrapper.write_word(&code);
                self.pop_attr();
            }
            Html(_html) => {}
            SoftBreak => {
                self.wrapper.write_line();
            }
            HardBreak => {
                self.wrapper.write_line();
            }
            Rule => {}
            FootnoteReference(_name) => {}
            TaskListMarker(true) => {}
            TaskListMarker(false) => {}
        }
    }
}

pub fn md<'a, S: AsRef<str>, T: Terminal + io::Write + 'a>(t: &'a mut T, content: S) {
    let mut f = LineFormatter::new(t, 0, 79);
    let parser = pulldown_cmark::Parser::new(content.as_ref());
    for event in parser {
        f.process_event(event);
    }
}
