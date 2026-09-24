#[derive(Clone, Copy)]
enum Quote {
    Single,
    Double,
}

#[derive(Default)]
struct LexicalState {
    quote: Option<Quote>,
    flow_depth: usize,
    block_indent: Option<usize>,
}

struct LineScan<'a> {
    line: &'a str,
    indent: usize,
    offset: usize,
    node_start: bool,
    after_quote: bool,
    mapping_value: bool,
    replacements: Vec<(usize, usize, String)>,
}

impl LexicalState {
    fn is_block_content(&mut self, line: &str, indent: usize) -> bool {
        let Some(parent_indent) = self.block_indent else {
            return false;
        };
        if line.trim().is_empty() || indent > parent_indent {
            return true;
        }
        self.block_indent = None;
        false
    }
}

impl<'a> LineScan<'a> {
    fn new(line: &'a str, indent: usize) -> Self {
        Self {
            line,
            indent,
            offset: 0,
            node_start: true,
            after_quote: false,
            mapping_value: false,
            replacements: Vec::new(),
        }
    }

    fn scan(mut self, state: &mut LexicalState) -> Vec<(usize, usize, String)> {
        while self.offset < self.line.len() {
            if let Some(quote) = state.quote {
                self.skip_quoted(state, quote);
            } else {
                self.scan_unquoted(state);
            }
        }
        self.replacements
    }

    fn skip_quoted(&mut self, state: &mut LexicalState, quote: Quote) {
        let bytes = self.line.as_bytes();
        let current = bytes[self.offset];
        let next = bytes.get(self.offset + 1).copied();
        match (quote, current, next) {
            (Quote::Double, b'\\', _) | (Quote::Single, b'\'', Some(b'\'')) => {
                self.offset += 2
            }
            (Quote::Double, b'"', _) | (Quote::Single, b'\'', _) => {
                state.quote = None;
                self.node_start = false;
                self.after_quote = true;
                self.offset += 1;
            }
            _ => self.offset += 1,
        }
    }

    fn scan_unquoted(&mut self, state: &mut LexicalState) {
        let current = self.line.as_bytes()[self.offset];
        if current == b'#' && self.starts_comment() {
            self.offset = self.line.len();
        } else if self.is_legacy_scalar(state) {
            self.replacements
                .push((self.offset, self.offset + 2, "'@@'".into()));
            self.node_start = false;
            self.offset += 2;
        } else if let Some((end, quoted)) = self.trailing_colon_value(state) {
            self.replacements.push((self.offset, end, quoted));
            self.node_start = false;
            self.offset = end;
        } else if self.is_block_header(state) {
            state.block_indent = Some(self.indent);
            self.offset = self.line.len();
        } else {
            self.advance_token(state, current);
            self.offset += 1;
        }
    }

    fn advance_token(&mut self, state: &mut LexicalState, current: u8) {
        let compact_flow_separator = state.flow_depth > 0 && self.after_quote;
        if !current.is_ascii_whitespace() {
            self.after_quote = false;
        }
        match current {
            b' ' | b'\t' => {}
            b'\'' if self.node_start => state.quote = Some(Quote::Single),
            b'"' if self.node_start => state.quote = Some(Quote::Double),
            b':' if self.separator_after(self.offset) || compact_flow_separator => {
                self.mapping_value = true;
                self.node_start = true;
            }
            b'[' | b'{' if self.node_start => {
                state.flow_depth += 1;
                self.node_start = true;
            }
            b']' | b'}' if state.flow_depth > 0 => {
                state.flow_depth -= 1;
                self.node_start = false;
            }
            b',' if state.flow_depth > 0 => self.node_start = true,
            b'-' | b'?' if self.node_start && self.separator_after(self.offset) => {}
            _ => self.node_start = false,
        }
    }

    fn starts_comment(&self) -> bool {
        self.offset == 0 || self.line.as_bytes()[self.offset - 1].is_ascii_whitespace()
    }

    fn separator_after(&self, offset: usize) -> bool {
        self.line
            .as_bytes()
            .get(offset + 1)
            .is_none_or(u8::is_ascii_whitespace)
    }

    fn is_legacy_scalar(&self, state: &LexicalState) -> bool {
        if !self.node_start || state.flow_depth != 0 {
            return false;
        }
        let tail = &self.line.as_bytes()[self.offset..];
        if !tail.starts_with(b"@@") {
            return false;
        }
        let is_key = self.offset == self.indent
            && tail.get(2) == Some(&b':')
            && self.separator_after(self.offset + 2);
        let is_value = self.mapping_value && scalar_tail(&self.line[self.offset + 2..]);
        is_key || is_value
    }

    fn trailing_colon_value(&self, state: &LexicalState) -> Option<(usize, String)> {
        if !self.node_start || !self.mapping_value || state.flow_depth != 0 {
            return None;
        }
        let tail = &self.line[self.offset..];
        if !tail.chars().next()?.is_alphanumeric() {
            return None;
        }
        let comment = tail
            .as_bytes()
            .windows(2)
            .position(|pair| pair[0].is_ascii_whitespace() && pair[1] == b'#')
            .map_or(tail.len(), |offset| offset + 1);
        let value = tail[..comment].trim_end_matches([' ', '\t']);
        if !value.ends_with(':') || value[..value.len() - 1].contains(':') {
            return None;
        }
        Some((
            self.offset + value.len(),
            format!("'{}'", value.replace('\'', "''")),
        ))
    }

    fn is_block_header(&self, state: &LexicalState) -> bool {
        let at_block_value = self.node_start && state.flow_depth == 0;
        if !at_block_value || !matches!(self.line.as_bytes()[self.offset], b'|' | b'>') {
            return false;
        }
        valid_block_suffix(&self.line[self.offset + 1..])
    }
}

fn scalar_tail(tail: &str) -> bool {
    if tail.is_empty() {
        return true;
    }
    if !tail.as_bytes()[0].is_ascii_whitespace() {
        return false;
    }
    let trimmed = tail.trim_start();
    trimmed.is_empty() || trimmed.starts_with('#')
}

fn valid_block_suffix(suffix: &str) -> bool {
    let modifiers = suffix.split_ascii_whitespace().next().unwrap_or("");
    if modifiers.starts_with('#') {
        return suffix.starts_with([' ', '\t']);
    }
    let mut sign_count = 0;
    let mut digit_count = 0;
    for byte in modifiers.bytes() {
        match byte {
            b'+' | b'-' => sign_count += 1,
            b'1'..=b'9' => digit_count += 1,
            _ => return false,
        }
    }
    let valid_modifiers = sign_count <= 1 && digit_count <= 1;
    valid_modifiers && scalar_tail(&suffix[modifiers.len()..])
}

fn append_quoted_tokens(
    output: &mut String,
    line: &str,
    replacements: &[(usize, usize, String)],
) {
    let mut copied_until = 0;
    for (start, end, quoted) in replacements {
        output.push_str(&line[copied_until..*start]);
        output.push_str(quoted);
        copied_until = *end;
    }
    output.push_str(&line[copied_until..]);
}

/// Quote bare `@@` block-mapping tokens and plain trailing-colon values.
///
/// This always runs before YAML parsing, never as a parse-error fallback. Quoted
/// strings, comments, block content and their line endings are left byte-identical.
/// Other invalid YAML (including bare `@@` in flow collections) is not repaired.
pub fn normalize_legacy_yaml(source: &str) -> String {
    let mut state = LexicalState::default();
    let mut output = String::with_capacity(source.len());
    for line in source.split_inclusive('\n') {
        let content = line.trim_end_matches(['\r', '\n']);
        let indent = content
            .bytes()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .count();
        if state.is_block_content(content, indent) {
            output.push_str(line);
            continue;
        }
        let offsets = LineScan::new(content, indent).scan(&mut state);
        append_quoted_tokens(&mut output, line, &offsets);
    }
    output
}
