//! Wikidot `%%token%%` values for one page, shared by live templates and ListPages.

use crate::utils::split_category;
use wikidot_forms::{FieldKind, FormSchema, Mapping, Value};

/// A form-category page's schema and stored values.
pub(super) struct FormRecord<'s> {
    pub schema: &'s FormSchema,
    pub values: Mapping,
}

/// Token sources for one page. Unknown or unavailable tokens stay literal.
pub(super) struct PageTokens<'a> {
    pub fullname: &'a str,
    pub title: &'a str,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub form: Option<&'a FormRecord<'a>>,
}

impl PageTokens<'_> {
    fn value(&self, token: &str) -> Option<String> {
        if let Some(field) = field_argument(token, "form_data") {
            return Some(self.form_data(field));
        }
        if let Some(field) = field_argument(token, "form_raw") {
            return Some(self.form_raw(field));
        }
        Some(match token {
            "name" => split_category(self.fullname).1.to_owned(),
            "fullname" => self.fullname.to_owned(),
            "title" => self.title.to_owned(),
            // An empty label renders the target's escaped title through FTML.
            "linked_title" | "title_linked" => format!("[[[{}|]]]", self.fullname),
            "link" => format!("/{}", self.fullname),
            "created_at" => format!("[[date {}]]", self.created_at?),
            "updated_at" => format!("[[date {}]]", self.updated_at?),
            _ => return None,
        })
    }

    fn form_raw(&self, field: &str) -> String {
        self.stored(field).map(scalar_text).unwrap_or_default()
    }

    /// Select fields display their option label; other fields display the stored value.
    fn form_data(&self, field: &str) -> String {
        let Some(form) = self.form else {
            return String::new();
        };
        let Some(value) = self.stored(field) else {
            return String::new();
        };
        let label = form
            .schema
            .fields
            .iter()
            .find(|candidate| {
                candidate.name == field && candidate.kind == FieldKind::Select
            })
            .and_then(|select| select.options.iter().find(|option| &option.code == value))
            .map(|option| &option.label);
        let kind = form
            .schema
            .fields
            .iter()
            .find(|candidate| candidate.name == field);
        let text = scalar_text(label.unwrap_or(value));
        match kind.map(|field| &field.kind) {
            Some(FieldKind::Wiki) => text,
            _ => literal_text(&text),
        }
    }

    fn stored(&self, field: &str) -> Option<&Value> {
        self.form?.values.get(field)
    }
}

fn field_argument<'t>(token: &'t str, function: &str) -> Option<&'t str> {
    token
        .strip_prefix(function)?
        .strip_prefix('{')?
        .strip_suffix('}')
}

/// Wikidot shows non-wiki form values as escaped text, so wrap each line in a raw span.
/// A line containing both raw delimiters (absent from the Cobalt archive) has its
/// `>@` split by a space.
fn literal_text(text: &str) -> String {
    let spans: Vec<_> = text
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| match (line.contains(">@"), line.contains("@@")) {
            (false, _) => format!("@<{line}>@"),
            (true, false) => format!("@@{line}@@"),
            (true, true) => format!("@<{}>@", line.replace(">@", "> @")),
        })
        .collect();
    spans.join(" ")
}

fn scalar_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        _ => String::new(),
    }
}

/// Replace every recognized `%%token%%`; unrecognized markers are copied unchanged.
pub(super) fn substitute_tokens(text: &str, tokens: &PageTokens) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("%%") {
        output.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let replacement = after
            .find("%%")
            .map(|end| &after[..end])
            .filter(|token| is_token_name(token))
            .and_then(|token| Some((token.len(), tokens.value(token)?)));
        match replacement {
            Some((length, value)) => {
                output.push_str(&value);
                rest = &after[length + 2..];
            }
            None => {
                output.push_str("%%");
                rest = after;
            }
        }
    }
    output.push_str(rest);
    output
}

fn is_token_name(token: &str) -> bool {
    let (name, argument) = match token.split_once('{') {
        Some((name, argument)) => (name, Some(argument)),
        None => (token, None),
    };
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
        && argument.is_none_or(|argument| {
            argument.strip_suffix('}').is_some_and(|field| {
                !field.is_empty()
                    && field
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wikidot_forms::{parse_schema, parse_values};

    const SCHEMA: &str = "fields:\n  rating:\n    type: select\n    values:\n      rated-t: T for Teen\n      rated-m: M for Mature\n  summary:\n    type: text\n  count:\n    type: text\n  hooks:\n    type: wiki\n";

    fn page<'a>(form: Option<&'a FormRecord<'a>>) -> PageTokens<'a> {
        PageTokens {
            fullname: "writing:letters-from-the-dead-1",
            title: "Letters from the dead [1]",
            created_at: Some(1_700_000_000),
            updated_at: None,
            form,
        }
    }

    #[test]
    fn substitutes_page_identity_and_links() {
        let output = substitute_tokens(
            "%%name%% %%fullname%% %%title%% %%linked_title%% %%title_linked%% %%link%% %%created_at%%",
            &page(None),
        );
        assert_eq!(
            output,
            "letters-from-the-dead-1 writing:letters-from-the-dead-1 Letters from the dead [1] \
             [[[writing:letters-from-the-dead-1|]]] [[[writing:letters-from-the-dead-1|]]] \
             /writing:letters-from-the-dead-1 [[date 1700000000]]",
        );
    }

    #[test]
    fn form_data_shows_literal_text_and_form_raw_shows_wikitext() {
        let schema = parse_schema(SCHEMA).unwrap();
        let form = FormRecord {
            schema: &schema,
            values: parse_values(
                "rating: rated-m\nsummary: 'A task -- to **help**'\ncount: 7\nhooks: '**Brave**'\n",
            )
            .unwrap(),
        };
        let output = substitute_tokens(
            "%%form_data{rating}%%|%%form_raw{rating}%%|%%form_data{summary}%%|%%form_raw{summary}%%|%%form_raw{count}%%|%%form_data{hooks}%%|%%form_data{missing}%%|",
            &page(Some(&form)),
        );
        assert_eq!(
            output,
            "@<M for Mature>@|rated-m|@<A task -- to **help**>@|A task -- to **help**|7|**Brave**||"
        );
    }

    #[test]
    fn literal_text_survives_raw_delimiters_and_line_breaks() {
        assert_eq!(literal_text("a >@ b"), "@@a >@ b@@");
        assert_eq!(literal_text("x@@y >@z"), "@<x@@y > @z>@");
        assert_eq!(literal_text("first\n\nsecond"), "@<first>@ @<second>@");
        assert_eq!(literal_text(""), "");
    }

    #[test]
    fn unknown_unavailable_and_malformed_markers_stay_literal() {
        let output = substitute_tokens(
            "%%content%% %%updated_at%% 100%% done %%form_data{}%% %%",
            &page(None),
        );
        assert_eq!(
            output,
            "%%content%% %%updated_at%% 100%% done %%form_data{}%% %%"
        );
    }

    #[test]
    fn a_literal_percent_pair_does_not_swallow_a_following_token() {
        assert_eq!(
            substitute_tokens("50%% %%name%%", &page(None)),
            "50%% letters-from-the-dead-1"
        );
    }
}
