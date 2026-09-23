//! Wikidot's form record format: `sfYaml::dump($values, 999)` with the Symfony
//! YAML inline scalar rules (PHP strings from the form POST), and the page
//! source trimmed on save, so no trailing newline.

use crate::{Mapping, Value};

pub(crate) fn dump_record(values: &Mapping) -> String {
    if values.is_empty() {
        return "{  }".into();
    }
    values
        .iter()
        .map(|(key, value)| format!("{}: {}", dump_scalar(key), dump_scalar(value)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn dump_scalar(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(value) => value.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => dump_string(text),
        // Records hold scalars only; validated before dumping.
        _ => unreachable!("form records contain only scalars"),
    }
}

/// `Inline::dump` for a PHP string, in its case order.
fn dump_string(text: &str) -> String {
    if is_ctype_digit(text) || is_php_numeric(text) {
        format!("'{text}'")
    } else if requires_double_quoting(text) {
        escape_with_double_quotes(text)
    } else if requires_single_quoting(text) {
        format!("'{}'", text.replace('\'', "''"))
    } else if text.is_empty() {
        "''".into()
    } else if is_timestamp(text)
        || ["null", "~", "true", "false"].contains(&text.to_lowercase().as_str())
    {
        format!("'{text}'")
    } else {
        text.into()
    }
}

fn is_ctype_digit(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// PHP `is_numeric` for strings: optional surrounding whitespace, sign, a
/// decimal number and an optional exponent.
fn is_php_numeric(text: &str) -> bool {
    let whitespace = |c: char| matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c');
    let body = text
        .trim_start_matches(whitespace)
        .trim_end_matches(whitespace);
    let body = body.strip_prefix(['+', '-']).unwrap_or(body);
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(index) => (&body[..index], Some(&body[index + 1..])),
        None => (body, None),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
    let mantissa_ok =
        digits(whole) && digits(fraction) && (!whole.is_empty() || !fraction.is_empty());
    let exponent_ok = exponent.is_none_or(|exponent| {
        let exponent = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
        is_ctype_digit(exponent)
    });
    mantissa_ok && exponent_ok
}

fn requires_double_quoting(text: &str) -> bool {
    text.chars().any(|c| {
        c <= '\x1f' || matches!(c, '\u{85}' | '\u{a0}' | '\u{2028}' | '\u{2029}')
    })
}

fn escape_with_double_quotes(text: &str) -> String {
    let mut output = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\0' => output.push_str("\\0"),
            '\x07' => output.push_str("\\a"),
            '\x08' => output.push_str("\\b"),
            '\t' => output.push_str("\\t"),
            '\n' => output.push_str("\\n"),
            '\x0b' => output.push_str("\\v"),
            '\x0c' => output.push_str("\\f"),
            '\r' => output.push_str("\\r"),
            '\x1b' => output.push_str("\\e"),
            '\u{85}' => output.push_str("\\N"),
            '\u{a0}' => output.push_str("\\_"),
            '\u{2028}' => output.push_str("\\L"),
            '\u{2029}' => output.push_str("\\P"),
            c if c <= '\x1f' => output.push_str(&format!("\\x{:02x}", c as u32)),
            c => output.push(c),
        }
    }
    output.push('"');
    output
}

fn requires_single_quoting(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(
            c,
            ' ' | '\t'
                | '\n'
                | '\r'
                | '\x0b'
                | '\x0c'
                | '\''
                | '"'
                | ':'
                | '{'
                | '}'
                | '['
                | ']'
                | ','
                | '&'
                | '*'
                | '#'
                | '?'
        )
    }) || text.starts_with(['-', '?', '|', '<', '>', '=', '!', '%', '@', '`'])
}

/// Symfony's timestamp pattern: `YYYY-M-D` with an optional time and zone.
fn is_timestamp(text: &str) -> bool {
    fn digits(text: &str, min: usize, max: usize) -> Option<&str> {
        let count = text.bytes().take_while(u8::is_ascii_digit).count();
        (min..=max).contains(&count).then(|| &text[count..])
    }
    let Some(rest) = digits(text, 4, 4)
        .and_then(|rest| rest.strip_prefix('-'))
        .and_then(|rest| digits(rest, 1, 2))
        .and_then(|rest| rest.strip_prefix('-'))
        .and_then(|rest| digits(rest, 1, 2))
    else {
        return false;
    };
    if rest.is_empty() {
        return true;
    }
    let rest = match rest.strip_prefix(['T', 't']) {
        Some(rest) => rest,
        None if rest.starts_with([' ', '\t']) => rest.trim_start_matches([' ', '\t']),
        None => return false,
    };
    let Some(mut rest) = digits(rest, 1, 2)
        .and_then(|rest| rest.strip_prefix(':'))
        .and_then(|rest| digits(rest, 2, 2))
        .and_then(|rest| rest.strip_prefix(':'))
        .and_then(|rest| digits(rest, 2, 2))
    else {
        return false;
    };
    if let Some(fraction) = rest.strip_prefix('.') {
        rest = fraction.trim_start_matches(|c: char| c.is_ascii_digit());
    }
    let rest = rest.trim_start_matches([' ', '\t']);
    if rest.is_empty() || rest == "Z" {
        return true;
    }
    rest.strip_prefix(['+', '-'])
        .and_then(|zone| digits(zone, 1, 2))
        .is_some_and(|zone| {
            zone.is_empty()
                || zone
                    .strip_prefix(':')
                    .and_then(|minutes| digits(minutes, 2, 2))
                    .is_some_and(str::is_empty)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(pairs: &[(&str, Value)]) -> Mapping {
        pairs
            .iter()
            .map(|(key, value)| (Value::String((*key).into()), value.clone()))
            .collect()
    }

    fn text(value: &str) -> Value {
        Value::String(value.into())
    }

    #[test]
    fn dumps_archived_character_values_like_wikidot() {
        // Lines from archived character:isabeau, character:morgauna and
        // character:slicket sources.
        let values = record(&[
            ("player", text("Jessa")),
            ("name", text("Isabeau Clark")),
            ("pronunciation", text("IZ-ah-boe")),
            ("aka", text("@@")),
            (
                "faction",
                text("House Thenedain, Stormwind\u{a0}House\u{a0}of\u{a0}Nobles"),
            ),
            ("portrait", text("SlicketThrottleblast2.jpg\t")),
            (
                "appearance",
                text("Valentine is 6'1\" tall.\nBuilt for speed."),
            ),
            ("count", text("7")),
            ("alignment", Value::Null),
        ]);
        assert_eq!(
            dump_record(&values),
            "player: Jessa\n\
             name: 'Isabeau Clark'\n\
             pronunciation: IZ-ah-boe\n\
             aka: '@@'\n\
             faction: \"House Thenedain, Stormwind\\_House\\_of\\_Nobles\"\n\
             portrait: \"SlicketThrottleblast2.jpg\\t\"\n\
             appearance: \"Valentine is 6'1\\\" tall.\\nBuilt for speed.\"\n\
             count: '7'\n\
             alignment: null",
        );
    }

    #[test]
    fn quotes_strings_yaml_would_read_as_other_types() {
        for (value, dumped) in [
            ("", "''"),
            ("true", "'true'"),
            ("Null", "'Null'"),
            ("1.5e3", "'1.5e3'"),
            ("2021-02-14", "'2021-02-14'"),
            ("2021-02-14 10:30:00", "'2021-02-14 10:30:00'"),
            ("-dash", "'-dash'"),
            ("it's", "'it''s'"),
            ("back\\slash\n", "\"back\\\\slash\\n\""),
            ("plain-word", "plain-word"),
        ] {
            assert_eq!(dump_string(value), dumped, "{value:?}");
        }
    }
}
