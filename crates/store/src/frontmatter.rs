use serde_yaml_ng::{Mapping, Value};

use crate::Date;

/// A file split into its YAML frontmatter and Markdown body.
///
/// Unknown keys are kept in `fields` so saving never drops data an agent or the
/// user added by hand.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Document {
    pub fields: Mapping,
    pub body: String,
    /// The raw YAML when the frontmatter block isn't a readable mapping
    /// (duplicate keys, a list, bad syntax). Such a file is never written.
    pub unreadable: Option<String>,
}

impl Document {
    /// Never fails: a missing or unterminated frontmatter block leaves
    /// `fields` empty so the Item still loads with defaults. An unparseable
    /// block also loads with defaults but keeps its text in `unreadable`, so
    /// callers can refuse to write the file instead of "fixing" it.
    pub fn parse(text: &str) -> Document {
        let Some((yaml, body)) = split(text) else {
            return Document {
                body: text.to_string(),
                ..Document::default()
            };
        };
        let (fields, unreadable) = match serde_yaml_ng::from_str::<Value>(yaml) {
            Ok(Value::Mapping(m)) => (m, None),
            // An empty or comment-only block.
            Ok(Value::Null) => (Mapping::new(), None),
            _ => (Mapping::new(), Some(yaml.to_string())),
        };
        Document {
            fields,
            body: body.to_string(),
            unreadable,
        }
    }

    pub fn render(&self) -> String {
        if let Some(raw) = &self.unreadable {
            return format!("---\n{raw}---\n{}", self.body);
        }
        if self.fields.is_empty() {
            return self.body.clone();
        }
        let yaml = serde_yaml_ng::to_string(&self.fields).unwrap_or_default();
        format!("---\n{yaml}---\n{}", self.body)
    }

    pub fn str(&self, key: &str) -> Option<String> {
        match self.fields.get(key)? {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }

    pub fn f64(&self, key: &str) -> Option<f64> {
        match self.fields.get(key)? {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
    }

    pub fn bool(&self, key: &str) -> Option<bool> {
        self.fields.get(key)?.as_bool()
    }

    pub fn date(&self, key: &str) -> Option<Date> {
        Date::parse(&self.str(key)?)
    }

    pub fn strings(&self, key: &str) -> Option<Vec<String>> {
        let seq = self.fields.get(key)?.as_sequence()?;
        let out: Vec<String> = seq
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        (!out.is_empty()).then_some(out)
    }

    pub fn set(&mut self, key: &str, value: impl Into<Value>) {
        self.fields.insert(Value::from(key), value.into());
    }

    pub fn set_opt(&mut self, key: &str, value: Option<impl Into<Value>>) {
        match value {
            Some(v) => self.set(key, v),
            None => {
                self.fields.remove(key);
            }
        }
    }
}

fn split(text: &str) -> Option<(&str, &str)> {
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return Some((&rest[..offset], &rest[offset + line.len()..]));
        }
        offset += line.len();
    }
    // A closing fence on the final line with no trailing newline.
    if rest[offset..].trim_end() == "---" {
        return Some((&rest[..offset], ""));
    }
    None
}
