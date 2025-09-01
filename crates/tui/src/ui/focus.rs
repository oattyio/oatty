//! Focus IDs and scaffolding for migrating to rat-focus.
//!
//! This module centralizes the focus node identifiers and documents the
//! intended parent/child focus hierarchy across the application. It is the
//! single source of truth for focus node names used by components. During the
//! migration to `rat-focus`, components will reference these IDs when
//! registering with the global focus store and when checking whether they are
//! focused for rendering.
//!
//! Phases (see migration plan):
//! - Phase 1: Root focus (palette, logs)
//! - Phase 2: Builder modal (search, commands, inputs)
//! - Phase 3: Table + pagination (grid, container + children)
//! - Phase 4: Event routing + scope trapping
//! - Phase 5: Visuals and cleanup (remove legacy enums)
//!
//! Note: This file intentionally has no direct dependency on `rat-focus` yet to
//! avoid introducing build changes until components are updated to implement
//! `HasFocus` and a global store is wired in.

/// Top-level/root focus nodes (always present).
pub mod root {
    /// Command palette input node ID.
    pub const PALETTE: &str = "root.palette";
    /// Logs pane node ID.
    pub const LOGS: &str = "root.logs";
}

/// Builder modal focus nodes (active within a trapped scope when visible).
pub mod builder {
    /// Search panel node ID.
    pub const SEARCH: &str = "builder.search";
    /// Command list panel node ID.
    pub const COMMANDS: &str = "builder.commands";
    /// Inputs panel node ID.
    pub const INPUTS: &str = "builder.inputs";
}

/// Table modal focus nodes.
pub mod table {
    /// Table grid node ID.
    pub const GRID: &str = "table.grid";
    /// Pagination container node ID (parent for the children below).
    pub const PAGINATION: &str = "table.pagination";

    /// Children within the pagination container.
    pub mod pagination {
        /// Range field list node ID.
        pub const RANGE_FIELD: &str = "table.pagination.range_field";
        /// Range start input node ID.
        pub const RANGE_START: &str = "table.pagination.range_start";
        /// Range end input node ID.
        pub const RANGE_END: &str = "table.pagination.range_end";
        /// Navigation controls node ID.
        pub const NAV: &str = "table.pagination.nav";
    }
}

/// Helper enum capturing all focus nodes for type safety inside the TUI crate.
///
/// This is optional sugar so callsites can match exhaustively; conversion to
/// `&'static str` is provided via `as_str()`.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeId {
    // root
    RootPalette,
    RootLogs,
    // builder
    BuilderSearch,
    BuilderCommands,
    BuilderInputs,
    // table grid + pagination
    TableGrid,
    TablePagination,
    TablePaginationRangeField,
    TablePaginationRangeStart,
    TablePaginationRangeEnd,
    TablePaginationNav,
}

impl NodeId {
    /// Returns the canonical string identifier for use with focus stores.
    pub const fn as_str(&self) -> &'static str {
        match self {
            // root
            NodeId::RootPalette => root::PALETTE,
            NodeId::RootLogs => root::LOGS,
            // builder
            NodeId::BuilderSearch => builder::SEARCH,
            NodeId::BuilderCommands => builder::COMMANDS,
            NodeId::BuilderInputs => builder::INPUTS,
            // table
            NodeId::TableGrid => table::GRID,
            NodeId::TablePagination => table::PAGINATION,
            NodeId::TablePaginationRangeField => table::pagination::RANGE_FIELD,
            NodeId::TablePaginationRangeStart => table::pagination::RANGE_START,
            NodeId::TablePaginationRangeEnd => table::pagination::RANGE_END,
            NodeId::TablePaginationNav => table::pagination::NAV,
        }
    }
}

/// Minimal focus store used during migration to integrate root focus and
/// support Tab/Shift-Tab traversal without introducing broad changes. This can
/// be adapted to delegate to `rat_focus` once components implement `HasFocus`.
#[derive(Debug, Default)]
pub struct FocusStore {
    // Stack of scopes; each scope is an ordered ring of focusable node IDs.
    scopes: Vec<Vec<&'static str>>,
    // Current index per scope (parallel to `scopes`).
    indices: Vec<usize>,
}

impl FocusStore {
    pub fn new() -> Self {
        Self {
            scopes: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Registers the root scope with the given ordered node IDs.
    pub fn register_root(&mut self, nodes: &[&'static str]) {
        self.scopes.clear();
        self.indices.clear();
        self.scopes.push(nodes.to_vec());
        self.indices.push(0);
    }

    /// Pushes a new trapped scope (e.g., a modal), focusing the first node.
    pub fn push_scope(&mut self, nodes: &[&'static str]) {
        self.scopes.push(nodes.to_vec());
        self.indices.push(0);
    }

    /// Pops the current scope, restoring the previous one.
    pub fn pop_scope(&mut self) {
        let _ = self.scopes.pop();
        let _ = self.indices.pop();
        if self.scopes.is_empty() {
            // Ensure at least an empty root exists to avoid panics on is_focused.
            self.scopes.push(Vec::new());
            self.indices.push(0);
        }
    }

    /// Returns the currently focused node ID, if any.
    pub fn current(&self) -> Option<&'static str> {
        if let (Some(scope), Some(idx)) = (self.scopes.last(), self.indices.last()) {
            return scope.get(*idx).copied();
        }
        None
    }

    /// Moves focus to the next node within the current scope.
    pub fn next(&mut self) {
        if let (Some(scope), Some(idx)) = (self.scopes.last(), self.indices.last_mut()) {
            if !scope.is_empty() {
                *idx = (*idx + 1) % scope.len();
            }
        }
    }

    /// Moves focus to the previous node within the current scope.
    pub fn prev(&mut self) {
        if let (Some(scope), Some(idx)) = (self.scopes.last(), self.indices.last_mut()) {
            if !scope.is_empty() {
                *idx = (*idx + scope.len() - 1) % scope.len();
            }
        }
    }

    /// Sets focus to a specific node within the current scope, if present.
    pub fn focus(&mut self, node: &'static str) {
        if let (Some(scope), Some(idx)) = (self.scopes.last(), self.indices.last_mut()) {
            if let Some(i) = scope.iter().position(|n| *n == node) {
                *idx = i;
            }
        }
    }

    /// Checks whether the given node is currently focused.
    pub fn is_focused(&self, node: &'static str) -> bool {
        self.current() == Some(node)
    }
}

/// Local trait mirroring the intent of `rat_focus::HasFocus`. Components can
/// implement this to expose their node ID during the migration period.
pub trait HasFocus {
    fn focus_id(&self) -> &'static str;
}

// Suggested skeleton for integrating `rat-focus` without breaking existing
// code:
// - Define a global focus store on `App` or in a shared context.
// - Register nodes and scopes at initialization:
//   - Root scope: `root.palette`, `root.logs`.
//   - Builder scope (trap): `builder.search`, `builder.commands`,
//     `builder.inputs`.
//   - Table scope (trap): `table.grid`, and pagination children.
// - Implement `HasFocus` for components, returning their node IDs.
// - Replace legacy boolean `focused` calculations in rendering with
//   `store.is_focused(node_id)`.
// - Replace Tab/BackTab logic with `store.next()/prev()` and scope-aware
//   routing.
