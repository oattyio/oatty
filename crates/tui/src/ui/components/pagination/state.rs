use heroku_types::Pagination;
use rat_focus::FocusFlag;
use ratatui::widgets::ListState;

/// State for pagination controls and range-based navigation.
#[derive(Debug, Default)]
pub struct PaginationState {
    /// Whether pagination controls are visible
    pub is_visible: bool,

    /// Current page number (0-based)
    pub current_page: usize,

    /// Total number of pages
    pub total_pages: usize,

    /// Items per page
    pub items_per_page: usize,

    /// Total number of items
    pub total_items: usize,

    /// Available range fields for the current command
    pub available_ranges: Vec<String>,

    /// Currently selected range field
    pub selected_range_field: Option<String>,

    /// Current range start value
    pub range_start: String,

    /// Current range end value
    pub range_end: String,
    /// Current sort order (asc/desc)
    pub order: Option<String>,
    /// Page size hint (max)
    pub max: usize,
    /// Raw Next-Range header for requesting next page
    pub next_range: Option<String>,

    /// Whether range mode is active
    pub range_mode: bool,

    /// List state for range field selection
    pub range_field_list_state: ListState,
    // rat-focus flags for subcontrols
    pub range_field_f: FocusFlag,
    pub range_start_f: FocusFlag,
    pub range_end_f: FocusFlag,
    pub nav_f: FocusFlag,
}

impl PaginationState {
    /// Creates a new pagination state
    pub fn new() -> Self {
        Self {
            is_visible: false,
            current_page: 0,
            total_pages: 0,
            items_per_page: 50,
            total_items: 0,
            available_ranges: Vec::new(),
            selected_range_field: None,
            range_start: String::new(),
            range_end: String::new(),
            order: None,
            max: 200,
            next_range: None,
            range_mode: false,
            range_field_list_state: ListState::default(),
            range_field_f: FocusFlag::named("table.pagination.range_field"),
            range_start_f: FocusFlag::named("table.pagination.range_start"),
            range_end_f: FocusFlag::named("table.pagination.range_end"),
            nav_f: FocusFlag::named("table.pagination.nav"),
        }
    }

    /// Sets the available range fields for the current command
    pub fn set_pagination(&mut self, pagination: Pagination) {
        // Auto-select first range field if none selected
        self.range_start = pagination.range_start;
        self.range_end = pagination.range_end;
        self.selected_range_field = Some(pagination.field);
        self.order = pagination.order;
        self.max = pagination.max;
        self.next_range = pagination.next_range;
        self.range_mode = true;
    }

    /// Populate available range fields; selects the first one if none chosen
    /// yet
    pub fn set_available_ranges(&mut self, ranges: Vec<String>) {
        self.available_ranges = ranges.clone();
        if self.selected_range_field.is_none() {
            if let Some(first) = ranges.first() {
                self.selected_range_field = Some(first.clone());
                self.range_field_list_state.select(Some(0));
            }
        } else if let Some(sel) = &self.selected_range_field {
            // Maintain selection index if possible
            if let Some(idx) = ranges.iter().position(|r| r == sel) {
                self.range_field_list_state.select(Some(idx));
            }
        }
    }

    /// Gets the current range field selection index
    pub fn selected_range_field_index(&self) -> Option<usize> {
        self.selected_range_field
            .as_ref()
            .and_then(|field| self.available_ranges.iter().position(|r| r == field))
    }

    /// Sets the selected range field by index
    pub fn set_selected_range_field_index(&mut self, index: usize) {
        if index < self.available_ranges.len() {
            self.selected_range_field = Some(self.available_ranges[index].clone());
        }
    }

    /// Moves to the next page
    pub fn next_page(&mut self) {
        if self.current_page < self.total_pages.saturating_sub(1) {
            self.current_page += 1;
        }
    }

    /// Moves to the previous page
    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
        }
    }

    /// Moves to the first page
    pub fn first_page(&mut self) {
        self.current_page = 0;
    }

    /// Moves to the last page
    pub fn last_page(&mut self) {
        self.current_page = self.total_pages.saturating_sub(1);
    }

    /// Gets the current page info as a string
    pub fn page_info(&self) -> String {
        if self.total_pages == 0 {
            "No pages".to_string()
        } else {
            format!("Page {} of {}", self.current_page + 1, self.total_pages)
        }
    }

    /// Gets the current range info as a string
    pub fn range_info(&self) -> String {
        if let Some(field) = &self.selected_range_field {
            if !self.range_start.is_empty() && !self.range_end.is_empty() {
                let mut s = format!("{}: {}..{}", field, self.range_start, self.range_end);
                if let Some(ord) = &self.order {
                    s.push_str(&format!("; order={}", ord));
                }
                if self.max > 0 {
                    s.push_str(&format!("; max={};", self.max));
                }
                s
            } else {
                format!("{}: (not set)", field)
            }
        } else {
            "No range field selected".to_string()
        }
    }

    /// Checks if there's a next page available
    pub fn has_next_page(&self) -> bool {
        // Prefer header-driven pagination: Next-Range presence indicates next page
        self.next_range.is_some()
    }

    /// Checks if there's a previous page available
    pub fn has_prev_page(&self) -> bool {
        self.current_page > 0
    }

    /// Resets pagination state
    pub fn reset(&mut self) {
        self.current_page = 0;
        self.total_pages = 0;
        self.total_items = 0;
        self.range_start.clear();
        self.range_end.clear();
        self.order = None;
        self.max = 200;
        self.next_range = None;
        // Focus flags persist; caller sets initial focus
    }
}
