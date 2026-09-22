use std::fmt;

/// Invalid delimiters, invalid YAML, or a shape not supported by this library.
#[derive(Debug)]
pub enum FormError {
    Delimiters(&'static str),
    Yaml {
        context: &'static str,
        source: serde_yaml_ng::Error,
    },
    Shape(String),
}

impl fmt::Display for FormError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Delimiters(message) => {
                write!(formatter, "invalid form delimiters: {message}")
            }
            Self::Yaml { context, source } => {
                write!(formatter, "invalid {context} YAML: {source}")
            }
            Self::Shape(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for FormError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Yaml { source, .. } => Some(source),
            _ => None,
        }
    }
}
