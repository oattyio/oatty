#[derive(Debug, Clone)]
pub struct EnvRow {
    pub key: String,
    pub value: String,
    pub is_secret: bool,
}

/// Editing field identifiers for a key/value row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyValueEditorField {
    /// Editing the key column of a row.
    Key,
    /// Editing the value column of a row.
    Value,
}
