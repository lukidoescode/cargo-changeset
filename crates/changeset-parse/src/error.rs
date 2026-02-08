use thiserror::Error;

#[derive(Debug, Error)]
pub enum FrontMatterError {
    #[error("missing opening delimiter '---'")]
    MissingOpeningDelimiter,

    #[error("missing closing delimiter '---'")]
    MissingClosingDelimiter,

    #[error("front matter is empty")]
    EmptyFrontMatter,
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("changeset must contain at least one release")]
    NoReleases,

    #[error("input exceeds maximum size of {max_bytes} bytes")]
    InputTooLarge { max_bytes: usize },
}

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("failed to parse YAML: {0}")]
    Yaml(#[from] serde_yml::Error),

    #[error(transparent)]
    FrontMatter(#[from] FrontMatterError),

    #[error(transparent)]
    Validation(#[from] ValidationError),
}
