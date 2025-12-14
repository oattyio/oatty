use oatty_types::{Effect, Pagination};
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use std::hash::{DefaultHasher, Hash, Hasher};

/// State for pagination controls and range-based navigation.
#[derive(Debug)]
pub struct PaginationState {
    /// Whether pagination controls are visible
    is_visible: bool,
    /// Whether the current page is the last page in the data set.
    last_page_hash: Option<u64>,
    /// Pagination history for the previous data sets
    pagination_history: Vec<Pagination>,
    /// Pagination information for the current data set
    pagination: Option<Pagination>,
    /// Map of Pagination objects whose next_range is treated as the previous range
    inverted_by_hash: Vec<u64>,
    /// Focus flags for individual navigation buttons
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
            last_page_hash: None,
            container_focus: FocusFlag::new().with_name("table.pagination.container"),
            nav_first_f: FocusFlag::new().with_name("table.pagination.nav.first"),
            nav_prev_f: FocusFlag::new().with_name("table.pagination.nav.prev"),
            nav_next_f: FocusFlag::new().with_name("table.pagination.nav.next"),
            nav_last_f: FocusFlag::new().with_name("table.pagination.nav.last"),
            last_area: Rect::default(),
            per_item_areas: vec![],
            pagination: None,
            pagination_history: vec![],
            inverted_by_hash: vec![],
        }
    }
}

impl PaginationState {
    pub fn should_reverse(&self, request_hash: u64) -> bool {
        self.inverted_by_hash.contains(&request_hash)
    }
    pub fn get_pagination(&self) -> Option<&Pagination> {
        self.pagination.as_ref()
    }
    /// Sets the available range fields for the current command
    pub fn set_pagination(&mut self, pagination: Option<Pagination>, request_hash: u64) {
        if pagination.is_none() {
            self.pagination = None;
            self.pagination_history.clear();
            self.inverted_by_hash.clear();
            self.is_visible = false;
            return;
        }
        let prev_pagination = self.pagination.take();
        let is_reversed = self.inverted_by_hash.contains(&request_hash);
        // if we're reversed, the previous page is
        // the next_range of the current pagination
        // and will need to know this later.
        if is_reversed {
            let prev = pagination.as_ref().unwrap();
            self.inverted_by_hash.push(self.get_hash(prev));
            self.pagination_history.push(prev.clone());
        }
        self.pagination = pagination;

        // only push history if we didn't request a previous page,
        if let Some(old) = prev_pagination
            && !is_reversed
        {
            self.pagination_history.push(old);
        }
        self.is_visible = true;
    }

    pub fn next_page(&mut self) -> Option<Effect> {
        let mut pagination = self.pagination.as_ref().cloned()?;
        let request_hash = self.get_hash(&pagination);
        let is_reversed = self.should_reverse(request_hash);
        if is_reversed {
            pagination = self.reverse_order(pagination);
        }
        self.inverted_by_hash.clear();
        Some(Effect::Run {
            hydrated_command: pagination.hydrated_shell_command?,
            range_override: pagination.next_range,
            request_hash,
        })
    }

    /// Moves to the previous page
    pub fn prev_page(&mut self) -> Option<Effect> {
        let pagination = self.pagination_history.pop()?;
        let request_hash = self.get_hash(&pagination);
        let is_reversed = self.should_reverse(request_hash);
        Some(Effect::Run {
            hydrated_command: pagination.hydrated_shell_command?,
            range_override: if is_reversed {
                pagination.next_range
            } else {
                pagination.this_range
            },
            request_hash,
        })
    }

    /// Moves to the first page
    pub fn first_page(&mut self) -> Option<Effect> {
        let pagination = self.pagination_history.first().cloned()?;
        self.inverted_by_hash.clear();
        self.pagination_history.clear();
        Some(Effect::Run {
            hydrated_command: pagination.hydrated_shell_command?,
            range_override: pagination.this_range,
            request_hash: 0,
        })
    }

    /// Composes a pagination object which reverses the
    /// ordering to get the last page in the data set.
    pub fn last_page(&mut self) -> Option<Effect> {
        let pagination = if let Some(pagination) = self.pagination_history.first().or(self.pagination.as_ref()).cloned().as_mut() {
            pagination.next_range = None;
            pagination.this_range = None;
            Some(self.reverse_order(pagination.clone()))
        } else {
            None
        }?;
        let request_hash = self.get_hash(&pagination);
        self.inverted_by_hash.push(request_hash);
        // last in wins with flags, so it's safe to have 2 --order flags
        let shell_command = pagination.hydrated_shell_command?;
        let order = pagination.order?;
        self.last_page_hash = Some(request_hash);
        Some(Effect::Run {
            hydrated_command: format!("{shell_command} --order {}", order),
            range_override: None,
            request_hash,
        })
    }

    /// Gets the current range info as a string
    pub fn range_info(&self) -> String {
        let Some(pagination) = &self.pagination else {
            return "No pagination info".to_string();
        };

        let Pagination {
            field,
            range_start,
            range_end,
            order,
            max,
            ..
        } = pagination;
        if !field.is_empty() {
            if !range_start.is_empty() && !range_end.is_empty() {
                let mut s = format!("{}: {}..{}", field, range_start, range_end);
                if let Some(ord) = &order {
                    s.push_str(&format!("; order={}", ord));
                }
                if *max > 0 {
                    s.push_str(&format!("; max={};", max));
                }
                s
            } else {
                format!("{}: (not set)", field)
            }
        } else {
            "No range field".to_string()
        }
    }
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    pub fn has_next_page(&self) -> bool {
        let has_next_range = self.pagination.as_ref().is_some_and(|p| p.next_range.is_some());
        if let (Some(last_page_hash), Some(pagination)) = (self.last_page_hash, self.pagination.as_ref()) {
            has_next_range && self.get_hash(pagination) != last_page_hash
        } else {
            has_next_range
        }
    }
    pub fn has_prev_page(&self) -> bool {
        !self.pagination_history.is_empty()
    }

    fn get_hash(&self, pagination: &Pagination) -> u64 {
        let mut hasher = DefaultHasher::new();
        pagination.hash(&mut hasher);
        hasher.finish()
    }

    fn reverse_order(&self, mut pagination: Pagination) -> Pagination {
        // use the first page to get the last page
        // by reversing the order. This requires reversing
        // the order of the results on the round trip.
        pagination.order = pagination.order.as_ref().or(Some(&"asc".to_string())).map(|ord| {
            // default is ascending, so we need to reverse it
            if ord.starts_with("desc") {
                "asc".to_string()
            } else {
                "desc".to_string()
            }
        });

        pagination
    }
}
impl HasFocus for PaginationState {
    fn build(&self, builder: &mut FocusBuilder) {
        if !self.is_visible {
            return;
        }
        let prev_enabled = !self.pagination_history.is_empty();
        let next_enabled = self.pagination.as_ref().is_some_and(|p| p.next_range.is_some());
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
