// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Static lexical pre-pass for the line-based Ruby DSL parsers (Gemfile, gemspec, Podfile, podspec).
//!
//! Blanks heredoc bodies and `=begin`/`=end` blocks (preserving line count) so their prose is not
//! matched as declarations, and finds `#` comments without cutting `#` inside string literals.

use std::collections::{HashMap, HashSet};
use std::ops::Range;

/// Ruby source split into lines, with heredoc bodies, embedded documents, and multi-line
/// literals located by one lexer pass over the whole file.
pub(crate) struct RubySource<'a> {
    lines: Vec<&'a str>,
    masked: Vec<bool>,
    code_starts: Vec<usize>,
    heredocs: Vec<Heredoc>,
}

struct Heredoc {
    opener_line: usize,
    body: Range<usize>,
    squiggly: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HeredocKind {
    /// `<<ID`: the terminator must start at column 0.
    Bare,
    /// `<<-ID`: the terminator may be indented.
    Dash,
    /// `<<~ID`: indented terminator, body is dedented.
    Squiggly,
}

struct HeredocOpener<'a> {
    id: &'a str,
    kind: HeredocKind,
    /// Byte length of the opener (`<<~"ID"`) on its line.
    len: usize,
}

/// Terminator-line lookup built lazily so lines without heredocs pay nothing.
/// Ruby allows leading whitespace only for `<<-`/`<<~` and never trailing whitespace (bar CR).
#[derive(Default)]
struct TerminatorIndex<'a> {
    by_indented: HashMap<&'a str, Vec<usize>>,
    by_exact: HashMap<&'a str, Vec<usize>>,
}

impl<'a> TerminatorIndex<'a> {
    fn build(lines: &[&'a str]) -> Self {
        let mut index = Self::default();
        for (i, line) in lines.iter().enumerate() {
            let line = line.strip_suffix('\r').unwrap_or(line);
            if line.trim().is_empty() {
                continue;
            }
            index
                .by_indented
                .entry(line.trim_start())
                .or_default()
                .push(i);
            index.by_exact.entry(line).or_default().push(i);
        }
        index
    }

    fn find(&self, id: &str, kind: HeredocKind, from: usize) -> Option<usize> {
        let map = match kind {
            HeredocKind::Bare => &self.by_exact,
            HeredocKind::Dash | HeredocKind::Squiggly => &self.by_indented,
        };
        let positions = map.get(id)?;
        positions
            .get(positions.partition_point(|&p| p < from))
            .copied()
    }
}

impl<'a> RubySource<'a> {
    pub(crate) fn parse(content: &'a str) -> Self {
        let lines: Vec<&str> = content.lines().collect();
        let (source, balanced) = Self::scan(&lines, true);
        // A literal still open at EOF means the file was mis-lexed; fall back to per-line state.
        if balanced {
            source
        } else {
            Self::scan(&lines, false).0
        }
    }

    /// Returns the scanned source and whether every literal was closed by the end of the file.
    fn scan(lines: &[&'a str], carry_state: bool) -> (Self, bool) {
        let mut masked = vec![false; lines.len()];
        let mut code_starts = vec![0; lines.len()];
        let mut heredocs = Vec::new();
        let mut terminators: Option<TerminatorIndex<'a>> = None;
        let mut embedded_doc_end_missing = false;
        let mut lexer = Lexer::default();

        let mut i = 0;
        while i < lines.len() {
            if !carry_state {
                lexer = Lexer::default();
            }
            let line = lines[i];

            if lexer.in_code()
                && !embedded_doc_end_missing
                && is_embedded_doc_marker(line, "=begin")
            {
                match (i + 1..lines.len()).find(|&j| is_embedded_doc_marker(lines[j], "=end")) {
                    Some(end) => {
                        masked[i..=end].fill(true);
                        i = end + 1;
                        continue;
                    }
                    None => embedded_doc_end_missing = true,
                }
            }

            let mut events = LineEvents::default();
            code_starts[i] = lexer.scan_line(line, &mut |_, _| true, &mut events);
            let openers = events.heredocs;

            let mut next = i + 1;
            if !openers.is_empty() {
                let index = terminators.get_or_insert_with(|| TerminatorIndex::build(lines));
                for opener in openers {
                    let Some(end) = index.find(opener.id, opener.kind, next) else {
                        break;
                    };
                    masked[next..=end].fill(true);
                    heredocs.push(Heredoc {
                        opener_line: i,
                        body: next..end,
                        squiggly: opener.kind == HeredocKind::Squiggly,
                    });
                    next = end + 1;
                }
            }
            i = next;
        }

        let source = Self {
            lines: lines.to_vec(),
            masked,
            code_starts,
            heredocs,
        };
        (source, lexer.in_code())
    }

    /// Source with heredoc bodies and embedded documents blanked, and continuations of multi-line
    /// literals cut from line starts so each line begins in code state for the per-line helpers.
    pub(crate) fn code(&self) -> String {
        let mut out = String::with_capacity(self.lines.iter().map(|l| l.len() + 1).sum());
        for ((line, masked), start) in self.lines.iter().zip(&self.masked).zip(&self.code_starts) {
            if !masked {
                out.push_str(line.get(*start..).unwrap_or_default());
            }
            out.push('\n');
        }
        out
    }

    /// Body of the first heredoc opened on `line_index`, dedented for `<<~` and trimmed.
    pub(crate) fn heredoc_body(&self, line_index: usize) -> Option<String> {
        let heredoc = self.heredocs.iter().find(|h| h.opener_line == line_index)?;
        let body = self.lines.get(heredoc.body.clone())?;
        let indent = if heredoc.squiggly {
            body.iter()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.len() - l.trim_start().len())
                .min()
                .unwrap_or(0)
        } else {
            0
        };
        let text = body
            .iter()
            .map(|l| l.get(indent..).unwrap_or_else(|| l.trim_start()))
            .collect::<Vec<_>>()
            .join("\n");
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_string())
    }
}

/// Convenience wrapper returning [`RubySource::code`].
pub(crate) fn mask_ruby_non_code(content: &str) -> String {
    RubySource::parse(content).code()
}

/// Returns the line without its trailing `#` comment, keeping `#` inside string literals.
pub(crate) fn strip_ruby_line_comment(line: &str) -> &str {
    match ruby_comment_start(line) {
        Some(pos) => &line[..pos],
        None => line,
    }
}

/// Byte offset of the first `#` that starts a comment, ignoring `#` inside string literals.
pub(crate) fn ruby_comment_start(line: &str) -> Option<usize> {
    let mut found = None;
    scan_code(line, |pos, rest| {
        if rest.starts_with('#') {
            found = Some(pos);
            return false;
        }
        true
    });
    found
}

/// Splits a line into statements at `;`, `do |x|`, `{ |x|`, `}` (of such a block), and `end`,
/// ignoring anything inside string literals.
pub(crate) fn ruby_statements(line: &str) -> Vec<&str> {
    let mut statements = Vec::new();
    let mut start = 0;
    let mut stop = line.len();
    let mut braces: Vec<bool> = Vec::new();
    scan_code(line, |pos, rest| {
        if pos < start {
            return true;
        }
        let boundary = if rest.starts_with('#') {
            stop = pos;
            return false;
        } else if rest.starts_with(';') || is_keyword_at(line, pos, "end") {
            Some(if rest.starts_with(';') { 1 } else { 3 })
        } else if is_keyword_at(line, pos, "do") {
            Some(2 + block_params_len(&rest[2..]))
        } else if let Some(after_brace) = rest.strip_prefix('{') {
            let params = block_params_len(after_brace);
            braces.push(params > 0);
            (params > 0).then_some(1 + params)
        } else if rest.starts_with('}') {
            braces.pop().unwrap_or(false).then_some(1)
        } else {
            None
        };
        if let Some(len) = boundary {
            statements.push(&line[start..pos]);
            start = pos + len;
        }
        true
    });
    if start < stop {
        statements.push(&line[start..stop]);
    }
    statements
        .into_iter()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

fn is_keyword_at(line: &str, pos: usize, keyword: &str) -> bool {
    let is_word = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '@' | '$');
    line[pos..].starts_with(keyword)
        && !line[..pos].chars().next_back().is_some_and(is_word)
        && !line[pos + keyword.len()..]
            .chars()
            .next()
            .is_some_and(|c| is_word(c) || matches!(c, '?' | '!'))
}

/// Length of an optional ` |params|` block-parameter list at the start of `rest`.
fn block_params_len(rest: &str) -> usize {
    let trimmed = rest.trim_start();
    let Some(params) = trimmed.strip_prefix('|') else {
        return 0;
    };
    params
        .find('|')
        .map_or(0, |end| rest.len() - trimmed.len() + end + 2)
}

fn is_embedded_doc_marker(line: &str, marker: &str) -> bool {
    line.strip_prefix(marker)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
}

fn parse_heredoc_opener(rest: &str) -> Option<HeredocOpener<'_>> {
    let after = rest.strip_prefix("<<")?;
    let (kind, after) = match after.as_bytes().first()? {
        b'~' => (HeredocKind::Squiggly, &after[1..]),
        b'-' => (HeredocKind::Dash, &after[1..]),
        _ => (HeredocKind::Bare, after),
    };
    let prefix_len = rest.len() - after.len();

    if let Some(quote) = after
        .chars()
        .next()
        .filter(|c| matches!(c, '\'' | '"' | '`'))
    {
        let inner = &after[1..];
        let id = &inner[..inner.find(quote)?];
        let len = prefix_len + id.len() + 2;
        return (!id.trim().is_empty()).then_some(HeredocOpener { id, kind, len });
    }

    let id_len = after
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(after.len());
    let id = &after[..id_len];
    let starts_ok = id
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    starts_ok.then_some(HeredocOpener {
        id,
        kind,
        len: prefix_len + id_len,
    })
}

/// Whether `<<` at `i` opens a heredoc (Ruby's rule) rather than being the shift operator.
fn starts_heredoc(bytes: &[u8], i: usize, prev: Token) -> bool {
    if bytes.get(i + 1) != Some(&b'<') {
        return false;
    }
    match prev {
        Token::ExprBeg => true,
        Token::Value => false,
        // Command argument: `foo <<~EOS` (space before, none after).
        Token::Identifier => {
            i > 0
                && bytes[i - 1].is_ascii_whitespace()
                && bytes.get(i + 2).is_some_and(|b| !b.is_ascii_whitespace())
        }
    }
}

/// Whether `rest` (just after a name) assigns to it, making the name a local variable.
fn is_assignment(rest: &str) -> bool {
    let rest = rest.trim_start();
    ["||=", "&&=", "+=", "-=", "*="]
        .iter()
        .any(|op| rest.starts_with(op))
        || (rest.starts_with('=') && !matches!(rest.as_bytes().get(1), Some(b'=' | b'~' | b'>')))
}

/// Calls `visit(offset, rest)` at each top-level code position of a single line (outside string,
/// percent, and regexp literals) until it returns false; the line is assumed to start in code.
fn scan_code<'a>(line: &'a str, mut visit: impl FnMut(usize, &'a str) -> bool) {
    Lexer::default().scan_line(line, &mut visit, &mut LineEvents::default());
}

/// Byte offsets of live `#{` interpolations: inside `"..."`, `%Q`/`%()`-style literals, unescaped.
pub(crate) fn ruby_interpolation_starts(line: &str) -> Vec<usize> {
    let mut events = LineEvents::default();
    Lexer::default().scan_line(line, &mut |_, _| true, &mut events);
    events.interpolations
}

/// What the lexer found on one line besides code positions.
#[derive(Default)]
struct LineEvents<'a> {
    interpolations: Vec<usize>,
    heredocs: Vec<HeredocOpener<'a>>,
}

/// Nesting cap for literals inside interpolations; deeper quotes are treated as plain code.
const MAX_LEXER_NESTING: usize = 8;

#[derive(Clone, Copy)]
enum Frame {
    Literal {
        open: u8,
        close: u8,
        interpolates: bool,
        depth: usize,
    },
    Interpolation {
        braces: usize,
    },
}

/// Ruby lexical state (open literals, `#{}` interpolations, and assigned local variable names)
/// that may carry across lines.
#[derive(Default)]
struct Lexer<'a> {
    stack: Vec<Frame>,
    locals: HashSet<&'a str>,
}

impl<'a> Lexer<'a> {
    fn in_code(&self) -> bool {
        self.stack.is_empty()
    }

    /// Lexes one line, calling `visit` at top-level code positions until it returns false or a
    /// comment starts. Returns the offset where the line first reaches top-level code.
    fn scan_line(
        &mut self,
        line: &'a str,
        visit: &mut dyn FnMut(usize, &'a str) -> bool,
        events: &mut LineEvents<'a>,
    ) -> usize {
        let bytes = line.as_bytes();
        let mut code_start = self.in_code().then_some(0);
        let mut prev = Token::ExprBeg;
        let mut after_dot = false;
        let mut word_start: Option<usize> = None;
        let last_slash = last_unescaped_slash(bytes);
        let mut i = 0;
        while i < bytes.len() {
            let byte = bytes[i];
            let nesting = self.stack.len();
            match self.stack.last_mut() {
                Some(Frame::Literal {
                    open,
                    close,
                    interpolates,
                    depth,
                }) => {
                    let (open, close, interpolates) = (*open, *close, *interpolates);
                    if byte == b'\\' {
                        i += 2;
                    } else if byte == close && *depth == 0 {
                        self.stack.pop();
                        prev = Token::Value;
                        i += 1;
                        if self.stack.is_empty() && code_start.is_none() {
                            code_start = Some(i);
                        }
                    } else if interpolates
                        && byte == b'#'
                        && bytes.get(i + 1) == Some(&b'{')
                        && nesting < MAX_LEXER_NESTING
                    {
                        events.interpolations.push(i);
                        self.stack.push(Frame::Interpolation { braces: 0 });
                        prev = Token::ExprBeg;
                        i += 2;
                    } else {
                        if byte == close {
                            *depth -= 1;
                        } else if byte == open && open != close {
                            *depth += 1;
                        }
                        i += 1;
                    }
                    continue;
                }
                Some(Frame::Interpolation { braces }) => {
                    if byte == b'}' && *braces == 0 {
                        self.stack.pop();
                        i += 1;
                        continue;
                    }
                    match byte {
                        b'{' => *braces += 1,
                        b'}' => *braces -= 1,
                        _ => {}
                    }
                }
                None => {}
            }

            if is_word_byte(byte) {
                word_start.get_or_insert(i);
            } else if let Some(start) = word_start.take() {
                let word = &line[start..i];
                prev = if after_dot {
                    Token::Identifier
                } else if self.locals.contains(word) {
                    Token::Value
                } else {
                    classify_word(word)
                };
                if prev == Token::Identifier && !after_dot && is_assignment(&line[i..]) {
                    self.locals.insert(word);
                }
                after_dot = false;
            }

            if word_start.is_none() {
                if byte == b'<'
                    && self.stack.is_empty()
                    && starts_heredoc(bytes, i, prev)
                    && let Some(opener) = parse_heredoc_opener(&line[i..])
                {
                    i += opener.len;
                    events.heredocs.push(opener);
                    prev = Token::Value;
                    continue;
                }
                if byte == b'?'
                    && prev == Token::ExprBeg
                    && let Some(end) = char_literal_end(line, i)
                {
                    prev = Token::Value;
                    i = end;
                    continue;
                }
                if self.stack.len() < MAX_LEXER_NESTING
                    && let Some((frame, next)) = literal_start(bytes, i, prev, last_slash)
                {
                    self.stack.push(frame);
                    i = next;
                    continue;
                }
            }
            if self.stack.is_empty() && (!visit(i, &line[i..]) || byte == b'#') {
                break;
            }
            if word_start.is_none() && !byte.is_ascii_whitespace() {
                prev = match byte {
                    b')' | b']' | b'}' => Token::Value,
                    _ => Token::ExprBeg,
                };
                after_dot = byte == b'.' && bytes.get(i + 1) != Some(&b'.');
            }
            i += line[i..].chars().next().map_or(1, char::len_utf8);
        }
        code_start.unwrap_or(line.len())
    }
}

/// Kind of the last significant token, which decides whether `/`, `%`, `?` start a literal.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Token {
    /// An expression may start here: line start, operator, opening bracket, or keyword.
    ExprBeg,
    /// A complete value: number, constant, variable, closing bracket, or literal.
    Value,
    /// A bare local name or method name, which may take a command argument (`foo %w[x]`).
    Identifier,
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'@' | b'$') || !byte.is_ascii()
}

fn classify_word(word: &str) -> Token {
    const EXPR_BEG_KEYWORDS: &[&str] = &[
        "and", "begin", "case", "do", "else", "elsif", "if", "in", "not", "or", "rescue", "return",
        "then", "unless", "until", "when", "while", "yield",
    ];
    const VALUE_KEYWORDS: &[&str] = &["class", "end", "false", "nil", "self", "true"];
    match word.as_bytes().first() {
        _ if EXPR_BEG_KEYWORDS.contains(&word) => Token::ExprBeg,
        _ if VALUE_KEYWORDS.contains(&word) => Token::Value,
        Some(first) if first.is_ascii_lowercase() || *first == b'_' => Token::Identifier,
        _ => Token::Value,
    }
}

/// Offset of the last `/` not preceded by a backslash, used to keep regexps on one line.
fn last_unescaped_slash(bytes: &[u8]) -> Option<usize> {
    (0..bytes.len())
        .rev()
        .find(|&p| bytes[p] == b'/' && (p == 0 || bytes[p - 1] != b'\\'))
}

/// The literal opened at `i` (quotes, `%q{}`-style, or `/regexp/`) and the offset after its opener.
fn literal_start(
    bytes: &[u8],
    i: usize,
    prev: Token,
    last_slash: Option<usize>,
) -> Option<(Frame, usize)> {
    let literal = |open: u8, close: u8, interpolates: bool| Frame::Literal {
        open,
        close,
        interpolates,
        depth: 0,
    };
    match bytes[i] {
        quote @ (b'"' | b'\'' | b'`') => Some((literal(quote, quote, quote != b'\''), i + 1)),
        // A regexp never spans lines here: without a closing `/` on this line it is division.
        b'/' if starts_literal(bytes, i, prev) && last_slash.is_some_and(|p| p > i) => {
            Some((literal(b'/', b'/', true), i + 1))
        }
        b'%' if starts_literal(bytes, i, prev) => {
            let (open_index, interpolates) = match *bytes.get(i + 1)? {
                b'q' | b'w' | b'i' | b's' => (i + 2, false),
                b'Q' | b'W' | b'I' | b'r' | b'x' => (i + 2, true),
                _ => (i + 1, true),
            };
            let open = *bytes.get(open_index)?;
            if open.is_ascii_alphanumeric()
                || open.is_ascii_whitespace()
                || open == b'='
                || !open.is_ascii()
            {
                return None;
            }
            let close = match open {
                b'{' => b'}',
                b'(' => b')',
                b'[' => b']',
                b'<' => b'>',
                other => other,
            };
            Some((literal(open, close, interpolates), open_index + 1))
        }
        _ => None,
    }
}

/// End of a `?X` / `?\X` character literal at `i` (in expression-start position), if any.
fn char_literal_end(line: &str, i: usize) -> Option<usize> {
    let mut chars = line.get(i + 1..)?.chars();
    let first = chars.next().filter(|c| !c.is_whitespace())?;
    let mut end = i + 1 + first.len_utf8();
    if first == '\\' {
        end += chars.next()?.len_utf8();
    }
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    (!line[end..].chars().next().is_some_and(is_ident)).then_some(end)
}

/// Whether `%` or `/` at `i` starts a literal rather than acting as a binary operator.
fn starts_literal(bytes: &[u8], i: usize, prev: Token) -> bool {
    let Some(&next) = bytes.get(i + 1) else {
        return false;
    };
    match prev {
        Token::ExprBeg => true,
        Token::Value => false,
        // Command argument: `foo /re/`, `s.description %q{}` (space before, none after).
        Token::Identifier => {
            i > 0
                && bytes[i - 1].is_ascii_whitespace()
                && !next.is_ascii_whitespace()
                && next != b'='
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_heredoc_bodies_and_keeps_line_count() {
        let content = "a = 1\ns.description = <<~DESC\n  gem \"fake\"\nDESC\nb = 2\n";
        let code = mask_ruby_non_code(content);
        assert_eq!(code, "a = 1\ns.description = <<~DESC\n\n\nb = 2\n");
        assert_eq!(code.lines().count(), content.lines().count());
    }

    #[test]
    fn masks_all_heredoc_forms() {
        let content = "x = <<-EOS\n  body1\n  EOS\ny = <<EOS\nbody2\nEOS\nz = <<~'END TEXT'\n  body3\n  END TEXT\nw = <<~\"Q\".strip\n body4\n Q\nkeep\n";
        let code = mask_ruby_non_code(content);
        for body in ["body1", "body2", "body3", "body4"] {
            assert!(!code.contains(body), "{body} should be masked: {code}");
        }
        assert!(code.ends_with("keep\n"));
    }

    #[test]
    fn masks_quoted_heredocs_with_any_delimiter() {
        let content = "s.description = <<'description'\ns.add_dependency \"fake\"\ndescription\nt = <<~\"text\"\n  s.name = \"fake\"\n  text\nkeep\n";
        let code = mask_ruby_non_code(content);
        assert!(!code.contains("fake"), "{code}");
        assert!(code.ends_with("keep\n"));
    }

    #[test]
    fn splits_statements_outside_strings() {
        assert_eq!(
            ruby_statements("s.subspec 'Core' do |ss| ss.dependency 'Foundation' end"),
            ["s.subspec 'Core'", "ss.dependency 'Foundation'"]
        );
        assert_eq!(
            ruby_statements("s.subspec('A') { |ss| ss.dependency 'B'; ss.dependency 'C', '1.0' }"),
            [
                "s.subspec('A')",
                "ss.dependency 'B'",
                "ss.dependency 'C', '1.0'"
            ]
        );
        assert_eq!(
            ruby_statements("puts \"s.dependency 'x'; do |y| end\""),
            ["puts \"s.dependency 'x'; do |y| end\""]
        );
        assert_eq!(
            ruby_statements("s.source = { :git => 'x', :tag => s.version.to_s }"),
            ["s.source = { :git => 'x', :tag => s.version.to_s }"]
        );
        assert_eq!(
            ruby_statements("s.endpoint = 'done'"),
            ["s.endpoint = 'done'"]
        );
    }

    #[test]
    fn heredoc_marker_inside_multiline_string_is_not_an_opener() {
        let content = "message = \"Example:\n<<~TEXT\n\"\ngem \"rails\"\nTEXT\n";
        assert_eq!(
            mask_ruby_non_code(content),
            "message = \"Example:\n\n\ngem \"rails\"\nTEXT\n"
        );
    }

    #[test]
    fn multiline_single_quoted_string_is_cut_from_code() {
        let content = "s.description = 'Usage:\n  s.dependency \"Fake\"; x = \"\n  done'; s.dependency 'Real'\n";
        assert_eq!(
            mask_ruby_non_code(content),
            "s.description = 'Usage:\n\n; s.dependency 'Real'\n"
        );
    }

    #[test]
    fn nested_literals_inside_interpolation_keep_quote_state() {
        let line = r##"puts "#{h["}"]}"; s.dependency 'Real'"##;
        assert_eq!(
            ruby_statements(line),
            [r##"puts "#{h["}"]}""##, "s.dependency 'Real'"]
        );
        assert_eq!(ruby_interpolation_starts(line), [6]);
        assert_eq!(
            ruby_statements(r##"x = "#{a ? '}' : %q{{}}}"; y = 1"##),
            [r##"x = "#{a ? '}' : %q{{}}}""##, "y = 1"]
        );
    }

    #[test]
    fn regexp_literals_hide_quotes() {
        assert_eq!(
            ruby_statements("files.reject { |f| f =~ /'/ }; s.dependency 'Real'"),
            ["files.reject", "f =~ /'/", "s.dependency 'Real'"]
        );
        assert_eq!(
            ruby_statements("x = a / b; y = 'c'"),
            ["x = a / b", "y = 'c'"]
        );
        assert_eq!(
            ruby_statements("x = a / b / c; s.dependency 'Real'"),
            ["x = a / b / c", "s.dependency 'Real'"]
        );
        assert_eq!(
            ruby_statements("ok = name =~ /;/ if foo /;/; s.dependency 'Real'"),
            ["ok = name =~ /;/ if foo /;/", "s.dependency 'Real'"]
        );
    }

    #[test]
    fn division_after_values_is_not_a_regexp() {
        let content = "half = 10 /2\ngem 'rails'\nthird = total[0] /3\nquarter = (n) /4\n";
        assert_eq!(mask_ruby_non_code(content), content);
        assert_eq!(
            ruby_statements("x = 10 /2; y = 'a/b'; s.dependency 'Real'"),
            ["x = 10 /2", "y = 'a/b'", "s.dependency 'Real'"]
        );
    }

    #[test]
    fn unclosed_regexp_is_never_carried_to_the_next_line() {
        let content = "x = foo /2\ngem 'rails'\ny = 1 / 2\n";
        assert_eq!(mask_ruby_non_code(content), content);
    }

    #[test]
    fn ternary_after_value_is_not_a_character_literal() {
        assert_eq!(
            ruby_statements("label = flag ?'!':'?'; s.dependency 'Real'"),
            ["label = flag ?'!':'?'", "s.dependency 'Real'"]
        );
        assert_eq!(
            ruby_statements("foo = bar ? 1 : 2; s.dependency 'Real'"),
            ["foo = bar ? 1 : 2", "s.dependency 'Real'"]
        );
        assert_eq!(
            ruby_statements("v = list[0] ?'a;b':'c'; w = 1"),
            ["v = list[0] ?'a;b':'c'", "w = 1"]
        );
    }

    #[test]
    fn unbalanced_file_falls_back_to_per_line_lexing() {
        let content = "x = \"unterminated\ngem \"rails\"\n";
        assert_eq!(mask_ruby_non_code(content), content);
    }

    #[test]
    fn character_literals_open_no_string() {
        let content = "q = ?'\ngem \"rails\"\nr = ?'\n";
        assert_eq!(mask_ruby_non_code(content), content);
        let content = "q = [?\", ?\\\"]\ngem 'rails'\nr = (?\")\n";
        assert_eq!(mask_ruby_non_code(content), content);
        assert_eq!(
            ruby_statements("x = ?; ; s.dependency 'Real'"),
            ["x = ?;", "s.dependency 'Real'"]
        );
    }

    #[test]
    fn ternaries_and_predicates_are_not_character_literals() {
        assert_eq!(
            ruby_statements("v = cond ? 'a;b' : 'c;d'; s.dependency 'Real'"),
            ["v = cond ? 'a;b' : 'c;d'", "s.dependency 'Real'"]
        );
        assert_eq!(
            ruby_statements("v = cond ?'a;b':'c'; s.dependency 'Real'"),
            ["v = cond ?'a;b':'c'", "s.dependency 'Real'"]
        );
        assert_eq!(
            ruby_statements("ok = valid?('x;y'); s.dependency 'Real'"),
            ["ok = valid?('x;y')", "s.dependency 'Real'"]
        );
    }

    #[test]
    fn finds_only_live_interpolations() {
        assert_eq!(ruby_interpolation_starts(r#""a#{b}" 'c#{d}'"#), [2]);
        assert_eq!(
            ruby_interpolation_starts(r#""a\#{b}" %q{#{c}}"#),
            Vec::<usize>::new()
        );
        assert_eq!(ruby_interpolation_starts("%Q(#{a}) %(#{b})"), [3, 11]);
    }

    #[test]
    fn terminator_with_trailing_space_does_not_end_heredoc() {
        let content = "x = <<~DESC\n  a\n  DESC \n  gem \"fake\"\n  DESC\nkeep\n";
        assert_eq!(mask_ruby_non_code(content), "x = <<~DESC\n\n\n\n\nkeep\n");
        let source = RubySource::parse(content);
        assert_eq!(
            source.heredoc_body(0).as_deref(),
            Some("a\nDESC \ngem \"fake\"")
        );
    }

    #[test]
    fn crlf_terminators_still_match() {
        let content =
            "x = <<-DESC\r\n  gem \"fake\"\r\n  DESC\r\ny = <<EOS\r\nfake\r\nEOS\r\nkeep\r\n";
        assert_eq!(
            mask_ruby_non_code(content),
            "x = <<-DESC\n\n\ny = <<EOS\n\n\nkeep\n"
        );
        assert_eq!(
            mask_ruby_non_code("x = <<EOS\nfake\nEOS\r"),
            "x = <<EOS\n\n\n"
        );
    }

    #[test]
    fn treats_percent_literals_as_strings() {
        assert_eq!(
            ruby_statements("s.description = %q{Example; s.dependency 'Fake'}"),
            ["s.description = %q{Example; s.dependency 'Fake'}"]
        );
        assert_eq!(
            ruby_statements("s.files = %w[a;b c]; s.dependency 'X'"),
            ["s.files = %w[a;b c]", "s.dependency 'X'"]
        );
        assert_eq!(
            ruby_statements("s.summary = %q{a {b}; c}; s.dependency 'Y'"),
            ["s.summary = %q{a {b}; c}", "s.dependency 'Y'"]
        );
        assert_eq!(
            ruby_statements("s.description %Q(see #{x}; end); s.dependency 'Z'"),
            ["s.description %Q(see #{x}; end)", "s.dependency 'Z'"]
        );
        assert_eq!(
            ruby_statements("s.summary = %|a; b|; v = x %= 2; y = a % b; z = 10%3"),
            ["s.summary = %|a; b|", "v = x %= 2", "y = a % b", "z = 10%3"]
        );
        assert_eq!(
            strip_ruby_line_comment("s.summary = %q{The #1 lib} # note"),
            "s.summary = %q{The #1 lib} "
        );
    }

    #[test]
    fn heredoc_or_shift_follows_the_previous_token() {
        let content = "s.description = <<description\ns.add_dependency 'Fake'\ndescription\nkeep\n";
        assert_eq!(
            mask_ruby_non_code(content),
            "s.description = <<description\n\n\nkeep\n"
        );

        let content =
            "x = <<~MyText\n  gem 'fake'\n  MyText\ncall_it <<-eos\n  gem 'fake'\n  eos\nkeep\n";
        assert_eq!(
            mask_ruby_non_code(content),
            "x = <<~MyText\n\n\ncall_it <<-eos\n\n\nkeep\n"
        );

        let shifts = "arr = []\narr <<value\nvalue\nclass << self\nself\n$LOAD_PATH <<lib\nlib\nn = 1 <<two\ntwo\n";
        assert_eq!(mask_ruby_non_code(shifts), shifts);
    }

    #[test]
    fn bare_heredoc_terminator_must_start_at_column_zero() {
        let content = "x = <<EOS\n  EOS\nbody\nEOS\nkeep\n";
        let code = mask_ruby_non_code(content);
        assert_eq!(code, "x = <<EOS\n\n\n\nkeep\n");
    }

    #[test]
    fn masks_multiple_heredocs_on_one_line() {
        let content = "call(<<~A, <<~B)\n  first\nA\n  second\nB\nkeep\n";
        let code = mask_ruby_non_code(content);
        assert_eq!(code, "call(<<~A, <<~B)\n\n\n\n\nkeep\n");
    }

    #[test]
    fn leaves_shift_operators_and_unterminated_openers_alone() {
        let content = "$LOAD_PATH << File.expand_path('lib')\nclass << self\nbuf <<value\nx = <<~NOPE\nkeep\n";
        assert_eq!(mask_ruby_non_code(content), content);
    }

    #[test]
    fn ignores_heredoc_markers_in_strings_and_comments() {
        let content = "puts \"use <<~EOS\"\n# <<~EOS\nkeep\nEOS\n";
        assert_eq!(mask_ruby_non_code(content), content);
    }

    #[test]
    fn masks_embedded_documents() {
        let content = "a\n=begin\ngem \"fake\"\n=end\nb\n";
        assert_eq!(mask_ruby_non_code(content), "a\n\n\n\nb\n");

        let unterminated = "a\n=begin\nb\n";
        assert_eq!(mask_ruby_non_code(unterminated), unterminated);

        let not_marker = "a\n=beginning\nb\n";
        assert_eq!(mask_ruby_non_code(not_marker), not_marker);
    }

    #[test]
    fn heredoc_body_dedents_squiggly_only() {
        let content = "s.description = <<~DESC\n    Line one\n      nested\n  DESC\nt = <<-DESC\n    raw\n      kept\n  DESC\n";
        let source = RubySource::parse(content);
        assert_eq!(
            source.heredoc_body(0).as_deref(),
            Some("Line one\n  nested")
        );
        assert_eq!(source.heredoc_body(4).as_deref(), Some("raw\n      kept"));
        assert_eq!(source.heredoc_body(1), None);
    }

    #[test]
    fn strips_comments_outside_strings_only() {
        assert_eq!(
            strip_ruby_line_comment("s.homepage = \"https://x/#{s.name}\" # note"),
            "s.homepage = \"https://x/#{s.name}\" "
        );
        assert_eq!(
            strip_ruby_line_comment("s.summary = 'The #1 lib'"),
            "s.summary = 'The #1 lib'"
        );
        assert_eq!(
            strip_ruby_line_comment("v = \"#{h[\"k\"]}\"#c"),
            "v = \"#{h[\"k\"]}\""
        );
        assert_eq!(strip_ruby_line_comment("# pod 'X'"), "");
        assert_eq!(strip_ruby_line_comment("pod 'X'"), "pod 'X'");
    }
}
