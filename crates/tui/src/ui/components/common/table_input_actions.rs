//! Shared table navigation helpers.
//!
//! This module centralizes key handling for table navigation so multiple
//! components can reuse consistent scrolling and focus behavior.

use crate::ui::components::results::ResultsTableState;
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use rat_focus::Focus;
use rat_focus::ratatui::layout::Rect;
use ratatui::layout::Position;

/// Handles standard table navigation keys.
///
/// Returns `true` when the key was consumed by navigation behavior.
///
/// # Arguments
///
/// * `key_code` - The keyboard key to handle.
/// * `table_state` - The table state to mutate for scrolling behavior.
/// * `focus` - The global focus ring used for Tab/BackTab traversal.
pub fn handle_table_navigation_key(key_code: KeyCode, state: &mut ResultsTableState, focus: &Focus) -> bool {
    let table_state = &mut state.table_state;
    match key_code {
        KeyCode::BackTab => {
            focus.prev();
        }
        KeyCode::Tab => {
            focus.next();
        }
        KeyCode::Up => {
            table_state.scroll_up_by(1);
        }
        KeyCode::Down => {
            table_state.scroll_down_by(1);
        }
        KeyCode::Left => {
            state.move_left();
        }
        KeyCode::Right => {
            state.move_right();
        }
        KeyCode::PageUp => {
            table_state.scroll_up_by(10);
        }
        KeyCode::PageDown => {
            table_state.scroll_down_by(10);
        }
        KeyCode::Home => {
            table_state.scroll_up_by(u16::MAX);
        }
        KeyCode::End => {
            table_state.scroll_down_by(u16::MAX);
        }
        _ => return false,
    }
    true
}
pub fn handle_table_mouse_actions(state: &mut ResultsTableState, mouse: MouseEvent, table_area: Rect) -> bool {
    let pos = Position {
        x: mouse.column,
        y: mouse.row,
    };
    let hit_test_table = table_area.contains(pos);
    // Update the mouse over index when the mouse moves or is released
    let maybe_table_index = find_table_index_from_pos(table_area, pos, state.table_state.offset(), hit_test_table);
    if mouse.kind == MouseEventKind::Moved || mouse.kind == MouseEventKind::Up(MouseButton::Left) {
        state.mouse_over_idx = maybe_table_index;
    }

    if hit_test_table {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) if hit_test_table => {
                state.table_state.select(maybe_table_index);
            }
            MouseEventKind::ScrollUp => {
                state.table_state.scroll_up_by(1);
            }
            MouseEventKind::ScrollDown => {
                state.table_state.scroll_down_by(1);
            }
            MouseEventKind::ScrollLeft => {
                state.table_state.scroll_left_by(1);
            }
            MouseEventKind::ScrollRight => {
                state.table_state.scroll_right_by(1);
            }
            _ => {
                return false;
            }
        }
    }

    true
}

pub fn find_table_index_from_pos(table_area: Rect, mouse_position: Position, list_offset: usize, hit_test_table: bool) -> Option<usize> {
    let idx = mouse_position.y.saturating_sub(table_area.y + 1) as usize + list_offset; // +1 for header
    if hit_test_table { Some(idx) } else { None }
}
