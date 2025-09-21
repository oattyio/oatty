#[derive(Debug, Clone)]
pub struct EnvRow {
    pub key: String,
    pub value: String,
    pub is_secret: bool,
}
