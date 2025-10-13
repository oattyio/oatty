use heroku_types::Pagination;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;

/// State for pagination controls and range-based navigation.
#[derive(Debug)]
pub struct PaginationState {
    /// Whether pagination controls are visible
    pub is_visible: bool,
    /// Current page number (0-based)
    pub current_page: usize,
    /// Whether a previous page exists in the pagination history.
    pub prev_available: bool,
    /// Whether additional pages can be requested using the Next-Range header.
    pub next_available: bool,
    /// The field used for pagination (e.g. id, name)
    pub field: String,
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
    /// rat-focus flags for individual navigation buttons
    pub container_focus: FocusFlag,
    pub nav_first_f: FocusFlag,
    pub nav_prev_f: FocusFlag,
    pub nav_next_f: FocusFlag,
    pub nav_last_f: FocusFlag,
    /// Mouse hit test params for button presses
    pub last_area: Rect,
    pub per_item_areas: Vec<Rect>,
}

impl Default for PaginationState {
    fn default() -> Self {
        Self {
            is_visible: false,
            current_page: 0,
            prev_available: false,
            next_available: false,
            field: String::new(),
            range_start: String::new(),
            range_end: String::new(),
            order: None,
            max: 200,
            next_range: None,
            range_mode: false,
            container_focus: FocusFlag::named("table.pagination.container"),
            nav_first_f: FocusFlag::named("table.pagination.nav.first"),
            nav_prev_f: FocusFlag::named("table.pagination.nav.prev"),
            nav_next_f: FocusFlag::named("table.pagination.nav.next"),
            nav_last_f: FocusFlag::named("table.pagination.nav.last"),
            last_area: Rect::default(),
            per_item_areas: vec![],
        }
    }
}

impl PaginationState {
    /// Sets the available range fields for the current command
    pub fn set_pagination(&mut self, pagination: Pagination) {
        // Auto-select first range field if none selected
        self.range_start = pagination.range_start;
        self.range_end = pagination.range_end;
        self.field = pagination.field;
        self.order = pagination.order;
        self.max = pagination.max;
        self.next_range = pagination.next_range;
        self.next_available = self.next_range.is_some();
        self.range_mode = true;
        self.prev_available = self.current_page > 0;
    }

    /// Moves to the previous page
    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
        }
        self.prev_available = self.current_page > 0;
    }

    /// Moves to the first page
    pub fn first_page(&mut self) {
        self.current_page = 0;
        self.prev_available = false;
    }

    /// Moves forward to represent navigating to the last page.
    pub fn last_page(&mut self) {
        if self.next_available {
            self.current_page = self.current_page.saturating_add(1);
            self.prev_available = true;
        }
    }

    /// Gets the current range info as a string
    pub fn range_info(&self) -> String {
        if !self.field.is_empty() {
            if !self.range_start.is_empty() && !self.range_end.is_empty() {
                let mut s = format!("{}: {}..{}", self.field, self.range_start, self.range_end);
                if let Some(ord) = &self.order {
                    s.push_str(&format!("; order={}", ord));
                }
                if self.max > 0 {
                    s.push_str(&format!("; max={};", self.max));
                }
                s
            } else {
                format!("{}: (not set)", self.field)
            }
        } else {
            "No range field".to_string()
        }
    }

    pub fn show_pagination(&mut self) {
        self.is_visible = true;
    }

    pub fn hide_pagination(&mut self) {
        self.is_visible = false;
    }
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }
}
impl HasFocus for PaginationState {
    fn build(&self, builder: &mut FocusBuilder) {
        if !self.is_visible {
            return;
        }
        let prev_enabled = self.prev_available;
        let next_enabled = self.next_available;
        let tag = builder.start(self);
        if prev_enabled {
            builder.leaf_widget(&self.nav_first_f);
            builder.leaf_widget(&self.nav_prev_f);
        }
        if next_enabled {
            builder.leaf_widget(&self.nav_next_f);
            builder.leaf_widget(&self.nav_last_f);
        }

        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
