use std::char;
use std::iter::Peekable;
use std::str::Chars;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ParseError {
    #[error("invalid escape sequence: \\{0}")]
    InvalidEscapeSequence(char),
    #[error("invalid unicode escape sequence")]
    InvalidUnicodeEscape,
    #[error("unexpected end of input")]
    UnexpectedEndOfInput,
    #[error("invalid hex digit in unicode escape")]
    InvalidHexDigit,
    #[error("unicode escape must have 1-6 digits in braces or exactly 4 digits")]
    InvalidUnicodeLength,
    #[error("string must start and end with \"")]
    MissingQuotes,
}

pub fn parse_string_literal(input: &str) -> Result<String, ParseError> {
    let mut chars = input.chars().peekable();
    let mut result = String::new();

    // check for opening quote
    if chars.next() != Some('"') {
        return Err(ParseError::MissingQuotes);
    }

    // parse content
    while let Some(c) = chars.next() {
        if c == '"' {
            // check if this is the closing quote (no more characters after)
            if chars.next().is_none() {
                return Ok(result);
            }
            return Err(ParseError::InvalidEscapeSequence('"'));
        }

        if c == '\\' {
            // handle escape sequence
            let escaped = chars.next().ok_or(ParseError::UnexpectedEndOfInput)?;
            match escaped {
                'n' => result.push('\n'),
                'r' => result.push('\r'),
                't' => result.push('\t'),
                '\\' => result.push('\\'),
                '\'' => result.push('\''),
                '"' => result.push('"'),
                'u' => result.push(parse_unicode_escape(&mut chars)?),
                _ => return Err(ParseError::InvalidEscapeSequence(escaped)),
            }
        } else {
            result.push(c);
        }
    }

    // if we get here, we never found a closing quote
    Err(ParseError::MissingQuotes)
}

fn parse_unicode_escape(chars: &mut Peekable<Chars>) -> Result<char, ParseError> {
    let mut hex_digits = String::new();

    if chars.peek() == Some(&'{') {
        // parse \u{XXXXXX} format
        chars.next(); // skip '{'

        while let Some(&c) = chars.peek() {
            if c == '}' {
                chars.next(); // skip '}'
                break;
            }
            if !c.is_ascii_hexdigit() {
                return Err(ParseError::InvalidHexDigit);
            }
            hex_digits.push(chars.next().unwrap());
        }

        // validate length (1-6 digits)
        if hex_digits.is_empty() || hex_digits.len() > 6 {
            return Err(ParseError::InvalidUnicodeLength);
        }
    } else {
        // parse \uXXXX format (exactly 4 digits)
        for _ in 0..4 {
            let c = chars.next().ok_or(ParseError::UnexpectedEndOfInput)?;
            if !c.is_ascii_hexdigit() {
                return Err(ParseError::InvalidHexDigit);
            }
            hex_digits.push(c);
        }
    }

    let code_point = u32::from_str_radix(&hex_digits, 16).map_err(|_| ParseError::InvalidHexDigit)?;
    char::from_u32(code_point).ok_or(ParseError::InvalidUnicodeEscape)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_string_literal() {
        assert_eq!(parse_string_literal(r#""foo""#), Ok("foo".to_string()));
        assert_eq!(parse_string_literal(r#""foo\nbar""#), Ok("foo\nbar".to_string()));
        assert_eq!(parse_string_literal(r#""foo\tbar""#), Ok("foo\tbar".to_string()));
        assert_eq!(parse_string_literal(r#""foo\"bar""#), Ok("foo\"bar".to_string()));
        assert_eq!(parse_string_literal(r#""foo\\bar""#), Ok("foo\\bar".to_string()));
        assert_eq!(parse_string_literal(r#""foo\u0034bar""#), Ok("foo4bar".to_string()));
        assert_eq!(parse_string_literal(r#""foo\u{0034}bar""#), Ok("foo4bar".to_string()));
        assert_eq!(parse_string_literal(r#""foo\u{1F600}bar""#), Ok("foo😀bar".to_string()));
        assert_eq!(parse_string_literal(r#""b\'a'r""#), Ok("b'a'r".to_string()));
        assert_eq!(parse_string_literal(r#""""#), Ok("".to_string()));

        assert_eq!(parse_string_literal("foo"), Err(ParseError::MissingQuotes));
        assert_eq!(parse_string_literal(r#""foo"bar"#), Err(ParseError::InvalidEscapeSequence('"')));
        assert_eq!(parse_string_literal(r#""foo\"#), Err(ParseError::UnexpectedEndOfInput));
        assert_eq!(parse_string_literal(r#""foo\""#), Err(ParseError::MissingQuotes));
        assert_eq!(parse_string_literal(r#""foo\u123""#), Err(ParseError::InvalidHexDigit));
        assert_eq!(parse_string_literal(r#""foo\u{123""#), Err(ParseError::InvalidHexDigit));
        assert_eq!(parse_string_literal(r#""foo\ug123""#), Err(ParseError::InvalidHexDigit));
        assert_eq!(parse_string_literal(r#""foo\u{}""#), Err(ParseError::InvalidUnicodeLength));
    }
}
