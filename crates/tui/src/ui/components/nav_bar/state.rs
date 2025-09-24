use heroku_types::Route;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;

/// A single item in the vertical navigation bar.
///
/// Each item consists of a display icon (typically a short symbol) and a
/// descriptive label used in tooltips, accessibility, and testing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavItem {
    /// Icon to display for the item (e.g., "$", "⌕", "{}").
    pub icon: String,
    /// Human-friendly description of the item (e.g., "Command").
    pub label: String,
    /// Route associated with this item
    pub route: Route,
}

impl NavItem {
    /// Creates a new navigation item.
    ///
    /// # Arguments
    /// - `icon`: The icon text to render. Prefer non-emoji symbols for
    ///   consistent terminal rendering.
    /// - `label`: A short descriptive label for the item.
    pub fn new(icon: impl Into<String>, label: impl Into<String>, route: Route) -> Self {
        Self {
            icon: icon.into(),
            label: label.into(),
            route,
        }
    }
}

/// State for the vertical navigation bar.
///
/// Owns the list of items, selection index, visibility, and rat-focus flags for
/// both the container and each item. Consumers can mutate state directly or via
/// the provided reducers to keep logic testable.
#[derive(Debug, Default, Clone)]
pub struct VerticalNavBarState {
    /// Whether the navbar is visible. The component does not enforce this,
    /// but consumers may choose to skip rendering when `false`.
    pub visible: bool,
    /// Items displayed in the navigation bar.
    pub items: Vec<NavItem>,
    /// Index of the currently selected item.
    pub selected_index: usize,
    /// Focus flag for the container in the global focus tree.
    pub container_focus: FocusFlag,
    /// Focus flags for each item; kept in sync with `items` length.
    pub item_focus_flags: Vec<FocusFlag>,
    /// Last rendered area of the nav bar; used for mouse focus and hit testing.
    pub last_area: Rect,
    /// Last computed per-item row areas for hit testing.
    pub per_item_areas: Vec<Rect>,
}

impl VerticalNavBarState {
    /// Creates a new vertical nav bar state with the provided items.
    ///
    /// Focus defaults to the first item if available.
    pub fn new(items: Vec<NavItem>) -> Self {
        let mut state = Self {
            visible: true,
            selected_index: 0,
            item_focus_flags: Vec::new(),
            container_focus: FocusFlag::named("nav.vertical"),
            items,
            last_area: Rect::default(),
            per_item_areas: Vec::new(),
        };
        state.rebuild_item_focus_flags();
        // Default focus to first item
        if !state.item_focus_flags.is_empty() {
            state.item_focus_flags[0].set(true);
        }
        state
    }

    /// Creates a nav bar pre-populated with typical application views.
    ///
    /// - Command: "$" (shell prompt)
    /// - Browser: "⌕" (search lens)
    /// - Plugins: "{}" (configuration/plugins)
    pub fn defaults_for_views() -> Self {
        Self::new(vec![
            NavItem::new("[Cmd]", "Command", Route::Palette),
            NavItem::new("[Brw]", "Browser", Route::Browser),
            NavItem::new("[Ext]", "Extensions", Route::Plugins),
        ])
    }

    /// Updates the collection of item focus flags to match `items` length.
    ///
    /// This preserves selection where possible; otherwise it clamps the
    /// `selected_index` into range and focuses that item.
    pub fn rebuild_item_focus_flags(&mut self) {
        let length = self.items.len();
        self.item_focus_flags = (0..length).map(|i| FocusFlag::named(&format!("nav.vertical.item.{i}"))).collect();
        if length == 0 {
            self.selected_index = 0;
        } else if self.selected_index >= length {
            self.selected_index = length - 1;
        }
        self.apply_selection_focus();
    }

    pub fn get_focused_list_item(&self) -> Option<(NavItem, usize)> {
        if let Some(idx) = self.item_focus_flags.iter().position(|l| l.get()) {
            return self.items.get(idx).and_then(|f| Some((f.clone(), idx)));
        }
        None
    }

    pub fn cycle_focus(&mut self, increment: bool) -> Option<FocusFlag> {
        let len = self.item_focus_flags.len();
        let ordinal = if increment { len + 1 } else { len - 1 };

        if let Some(idx) = self.item_focus_flags.iter().position(|l| l.get()) {
            let sum = idx + ordinal;
            let new_index = sum % len;

            return self.item_focus_flags.get(new_index).cloned();
        }
        None
    }

    pub fn set_route(&mut self, route: Route) -> Route {
        if let Some(idx) = self.items.iter().position(|r| r.route == route) {
            self.selected_index = idx;
        }
        route
    }

    /// Applies the current selection to the item focus flags.
    fn apply_selection_focus(&mut self) {
        for (i, flag) in self.item_focus_flags.iter().enumerate() {
            flag.set(i == self.selected_index);
        }
    }
}

impl HasFocus for VerticalNavBarState {
    /// Builds a focus subtree consisting of each item as a leaf under the
    /// container focus flag.
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        for flag in &self.item_focus_flags {
            builder.leaf_widget(flag);
        }
        builder.end(tag);
    }

    /// Returns the container focus flag for the nav bar.
    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    /// Returns the last rendered area for mouse focus integration.
    fn area(&self) -> Rect {
        self.last_area
    }
}
