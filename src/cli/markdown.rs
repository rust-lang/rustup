// Write Markdown to the terminal
use std::io::Write;

use pulldown_cmark::{Event, Tag, TagEnd};

use crate::process::terminalsource::{Attr, Color, ColorableTerminal};

// Handles the wrapping of text written to the console
struct LineWrapper<'a> {
    indent: u32,
    margin: u32,
    pos: u32,
    w: &'a mut ColorableTerminal,
}

impl<'a> LineWrapper<'a> {
    // Just write a newline
    fn write_line(&mut self) {
        let _ = writeln!(self.w.lock());
        // Reset column position to start of line
        self.pos = 0;
    }
    // Called before writing text to ensure indent is applied
    fn write_indent(&mut self) {
        if self.pos == 0 {
            // Write a space for each level of indent
            for _ in 0..self.indent {
                let _ = write!(self.w.lock(), " ");
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
        let _ = write!(self.w.lock(), "{word}");
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
    // Writes code block where each line is indented
    fn write_code_block(&mut self, text: &str) {
        for line in text.lines() {
            self.write_word(line); // Will call write_indent()
            self.write_line();
        }
    }
    // Constructor
    fn new(w: &'a mut ColorableTerminal, indent: u32, margin: u32) -> Self {
        LineWrapper {
            indent,
            margin,
            pos: indent,
            w,
        }
    }
}

// Handles the formatting of text
struct LineFormatter<'a> {
    is_code_block: bool,
    wrapper: LineWrapper<'a>,
    attrs: Vec<Attr>,
}

impl<'a> LineFormatter<'a> {
    fn new(w: &'a mut ColorableTerminal, indent: u32, margin: u32) -> Self {
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
            Tag::Heading { .. } => {
                self.push_attr(Attr::Bold);
                self.wrapper.write_line();
            }
            Tag::MetadataBlock(_) => {}
            Tag::Table(_alignments) => {}
            Tag::TableHead => {}
            Tag::TableRow => {}
            Tag::TableCell => {}
            Tag::BlockQuote(_) => {}
            Tag::DefinitionList => {
                self.wrapper.write_line();
                self.wrapper.indent += 2;
            }
            Tag::DefinitionListTitle | Tag::DefinitionListDefinition => {}
            Tag::CodeBlock(_) | Tag::HtmlBlock { .. } => {
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
                self.push_attr(Attr::ForegroundColor(Color::Red));
            }
            Tag::Strong => {}
            Tag::Strikethrough => {}
            Tag::Link { .. } => {}
            Tag::Image { .. } => {}
            Tag::FootnoteDefinition(_name) => {}
            Tag::Superscript => {}
            Tag::Subscript => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                self.wrapper.write_line();
            }
            TagEnd::Heading { .. } => {
                self.wrapper.write_line();
                self.pop_attr();
            }
            TagEnd::Table => {}
            TagEnd::TableHead => {}
            TagEnd::TableRow => {}
            TagEnd::TableCell => {}
            TagEnd::BlockQuote(_) => {}
            TagEnd::DefinitionList => {
                self.wrapper.indent -= 2;
                self.wrapper.write_line();
            }
            TagEnd::DefinitionListTitle | TagEnd::DefinitionListDefinition => {}
            TagEnd::CodeBlock | TagEnd::HtmlBlock => {
                self.is_code_block = false;
                self.wrapper.indent -= 2;
            }
            TagEnd::List(_) => {
                self.wrapper.indent -= 2;
                self.wrapper.write_line();
            }
            TagEnd::Item => {}
            TagEnd::Emphasis => {
                self.pop_attr();
            }
            TagEnd::Strong => {}
            TagEnd::Strikethrough => {}
            TagEnd::Link => {}
            TagEnd::Image => {} // shouldn't happen, handled in start
            TagEnd::FootnoteDefinition => {}
            TagEnd::MetadataBlock(_) => {}
            TagEnd::Superscript => {}
            TagEnd::Subscript => {}
        }
    }

    fn process_event(&mut self, event: Event<'a>) {
        use self::Event::*;
        match event {
            Start(tag) => self.start_tag(tag),
            End(tag) => self.end_tag(tag),
            Text(text) => {
                if self.is_code_block {
                    self.wrapper.write_code_block(&text);
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
            InlineHtml(_) => {}
            InlineMath(_) => {}
            DisplayMath(_) => {}
        }
    }
}

pub(crate) fn md<S: AsRef<str>>(t: &mut ColorableTerminal, content: S) {
    let mut f = LineFormatter::new(t, 0, 79);
    let parser = pulldown_cmark::Parser::new(content.as_ref());
    for event in parser {
        f.process_event(event);
    }
}
