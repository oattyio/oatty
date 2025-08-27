/// The main application state containing all UI data and business logic.
///
/// This struct serves as the central state container for the entire TUI
/// application, managing user interactions, data flow, and UI state.
#[derive(Debug)]
pub struct LogsState {
    pub entries: Vec<String>,
}

impl Default for LogsState {
    fn default() -> Self {
        LogsState {
            entries: vec!["Welcome to Heroku TUI".into()],
        }
    }
}
