
#[derive(Debug, Default)]
pub struct TableState {
    pub show: bool,
    pub offset: usize,
    pub result_json: Option<serde_json::Value>,
}