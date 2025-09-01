use crossterm::event::{KeyCode, KeyEvent};
use heroku_types::Pagination;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::*,
    text::{Line, Span},
    widgets::*,
};

use crate::ui::components::component::Component;
use crate::ui::theme::roles::Theme as UiTheme;
use crate::ui::theme::helpers as th;
use super::state::{PaginationState, PaginationFocus};

/// Pagination component for range-based navigation and controls.
///
/// This component provides:
/// - Range field selection using a List widget
/// - Range start/end input fields
/// - Navigation controls (prev/next/first/last)
/// - Page information display
/// - Integration with the existing theme system
#[derive(Default)]
pub struct PaginationComponent {
    state: PaginationState,
}

impl PaginationComponent {
    /// Creates a new pagination component
    pub fn new() -> Self {
        Self {
            state: PaginationState::new(),
        }
    }
    
    /// Gets a mutable reference to the pagination state
    pub fn state_mut(&mut self) -> &mut PaginationState {
        &mut self.state
    }
    
    /// Gets a reference to the pagination state
    pub fn state(&self) -> &PaginationState {
        &self.state
    }
    
    /// Sets the available range fields for the current command
    pub fn set_pagination(&mut self, pagination: Pagination) {
        self.state.set_pagination(pagination);
    }

    /// Sets the available range fields list
    pub fn set_available_ranges(&mut self, ranges: Vec<String>) {
        self.state.set_available_ranges(ranges);
    }
    
    /// Shows the pagination controls
    pub fn show(&mut self) {
        self.state.is_visible = true;
    }
    
    /// Hides the pagination controls
    pub fn hide(&mut self) {
        self.state.is_visible = false;
    }
    
    /// Renders the pagination controls
    pub fn render(&mut self, frame: &mut Frame, area: Rect, theme: &dyn UiTheme) {
        if !self.state.is_visible {
            return;
        }
        
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Range controls
                Constraint::Length(1), // Divider
                Constraint::Length(3), // Navigation controls
            ])
            .split(area);
        
        self.render_range_controls(frame, chunks[0], theme);
        self.render_divider(frame, chunks[1], theme);
        self.render_navigation_controls(frame, chunks[2], theme);
    }
    
    /// Renders the range field selection and input controls
    fn render_range_controls(&mut self, frame: &mut Frame, area: Rect, theme: &dyn UiTheme) {
        if !self.state.range_mode {
            return;
        }
        
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(15), // Range field list
                Constraint::Length(1),  // Spacer
                Constraint::Length(20), // Range start input
                Constraint::Length(1),  // Spacer
                Constraint::Length(20), // Range end input
            ])
            .split(area);
        
        // Range field selection list
        self.render_range_field_list(frame, chunks[0], theme);
        
        // Range start input
        self.render_range_input(
            frame, 
            chunks[2], 
            "Start", 
            &self.state.range_start,
            self.state.focus == PaginationFocus::RangeStart,
            theme
        );
        
        // Range end input
        self.render_range_input(
            frame, 
            chunks[4], 
            "End", 
            &self.state.range_end,
            self.state.focus == PaginationFocus::RangeEnd,
            theme
        );
    }
    
    /// Renders the range field selection list
    fn render_range_field_list(&mut self, frame: &mut Frame, area: Rect, theme: &dyn UiTheme) {
        let title = "Range Field";
        let title_style = if self.state.focus == PaginationFocus::RangeField {
            theme.accent_emphasis_style()
        } else {
            theme.text_secondary_style()
        };
        
        let items: Vec<ListItem> = self.state.available_ranges
            .iter()
            .map(|field| {
                let style = if self.state.selected_range_field.as_ref() == Some(field) {
                    theme.selection_style()
                } else {
                    theme.text_primary_style()
                };
                ListItem::new(field.clone()).style(style)
            })
            .collect();
        
        let list = List::new(items)
            .block(
                Block::default()
                    .title(Line::from(vec![
                        Span::styled(title, title_style),
                    ]))
                    .borders(Borders::ALL)
                    .border_style(theme.border_style(self.state.focus == PaginationFocus::RangeField))
            )
            .style(theme.text_primary_style());
        
        frame.render_stateful_widget(list, area, &mut self.state.range_field_list_state);
    }
    
    /// Renders a range input field
    fn render_range_input(
        &self,
        frame: &mut Frame,
        area: Rect,
        label: &str,
        value: &str,
        focused: bool,
        theme: &dyn UiTheme,
    ) {
        let title_style = if focused {
            theme.accent_emphasis_style()
        } else {
            theme.text_secondary_style()
        };
        
        let input = Paragraph::new(value)
            .block(
                Block::default()
                    .title(Line::from(vec![
                        Span::styled(label, title_style),
                    ]))
                    .borders(Borders::ALL)
                    .border_style(theme.border_style(focused))
            )
            .style(theme.text_primary_style());
        
        frame.render_widget(input, area);
    }
    
    /// Renders a divider line
    fn render_divider(&self, frame: &mut Frame, area: Rect, theme: &dyn UiTheme) {
        let divider = Line::from(vec![
            Span::styled("─".repeat(area.width as usize), theme.text_muted_style()),
        ]);
        let paragraph = Paragraph::new(divider);
        frame.render_widget(paragraph, area);
    }
    
    /// Renders the navigation controls
    fn render_navigation_controls(&self, frame: &mut Frame, area: Rect, theme: &dyn UiTheme) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(8),  // First button
                Constraint::Length(1),  // Spacer
                Constraint::Length(8),  // Prev button
                Constraint::Length(1),  // Spacer
                Constraint::Min(0),     // Page info
                Constraint::Length(1),  // Spacer
                Constraint::Length(8),  // Next button
                Constraint::Length(1),  // Spacer
                Constraint::Length(8),  // Last button
            ])
            .split(area);
        
        // First page button
        self.render_nav_button(
            frame,
            chunks[0],
            "First",
            self.state.has_prev_page(),
            self.state.focus == PaginationFocus::Navigation,
            theme,
        );
        
        // Previous page button
        self.render_nav_button(
            frame,
            chunks[2],
            "Prev",
            self.state.has_prev_page(),
            self.state.focus == PaginationFocus::Navigation,
            theme,
        );
        
        // Page info
        let page_info = self.state.page_info();
        let range_info = if self.state.range_mode {
            format!(" | {}", self.state.range_info())
        } else {
            String::new()
        };
        
        let info_text = format!("{}{}", page_info, range_info);
        let info_paragraph = Paragraph::new(info_text)
            .style(theme.text_secondary_style())
            .alignment(Alignment::Center);
        frame.render_widget(info_paragraph, chunks[4]);
        
        // Next page button
        self.render_nav_button(
            frame,
            chunks[6],
            "Next",
            self.state.has_next_page(),
            self.state.focus == PaginationFocus::Navigation,
            theme,
        );
        
        // Last page button
        self.render_nav_button(
            frame,
            chunks[8],
            "Last",
            self.state.has_next_page(),
            self.state.focus == PaginationFocus::Navigation,
            theme,
        );
    }
    
    /// Renders a navigation button
    fn render_nav_button(
        &self,
        frame: &mut Frame,
        area: Rect,
        label: &str,
        enabled: bool,
        focused: bool,
        theme: &dyn UiTheme,
    ) {
        let button_style = if enabled {
            if focused {
                th::button_primary_style(theme, true)
            } else {
                th::button_secondary_style(theme, true)
            }
        } else {
            th::button_secondary_style(theme, false)
        };
        
        let button = Paragraph::new(label)
            .block(Block::default().borders(Borders::ALL))
            .style(button_style)
            .alignment(Alignment::Center);
        
        frame.render_widget(button, area);
    }
}

impl Component for PaginationComponent {
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut crate::app::App) {
        self.render(frame, rect, &*app.ctx.theme);
    }
    
    fn handle_key_events(&mut self, _app: &mut crate::app::App, event: KeyEvent) -> Vec<crate::app::Effect> {
        if !self.state.is_visible {
            return vec![];
        }
        
        match event.code {
            KeyCode::Tab => {
                // Cycle through focus areas
                self.state.focus = match self.state.focus {
                    PaginationFocus::RangeField => PaginationFocus::RangeStart,
                    PaginationFocus::RangeStart => PaginationFocus::RangeEnd,
                    PaginationFocus::RangeEnd => PaginationFocus::Navigation,
                    PaginationFocus::Navigation => PaginationFocus::RangeField,
                };
            }
            KeyCode::BackTab => {
                // Cycle backwards through focus areas
                self.state.focus = match self.state.focus {
                    PaginationFocus::RangeField => PaginationFocus::Navigation,
                    PaginationFocus::RangeStart => PaginationFocus::RangeField,
                    PaginationFocus::RangeEnd => PaginationFocus::RangeStart,
                    PaginationFocus::Navigation => PaginationFocus::RangeEnd,
                };
            }
            KeyCode::Up | KeyCode::Down => {
                if self.state.focus == PaginationFocus::RangeField {
                    // Handle range field list navigation
                    let current_index = self.state.selected_range_field_index().unwrap_or(0);
                    let new_index = match event.code {
                        KeyCode::Up => current_index.saturating_sub(1),
                        KeyCode::Down => (current_index + 1).min(self.state.available_ranges.len().saturating_sub(1)),
                        _ => current_index,
                    };
                    self.state.set_selected_range_field_index(new_index);
                    self.state.range_field_list_state.select(Some(new_index));
                }
            }
            KeyCode::Left | KeyCode::Right => {
                if self.state.focus == PaginationFocus::Navigation {
                    // Handle navigation button selection (could be extended for button highlighting)
                    match event.code {
                        KeyCode::Left => self.state.prev_page(),
                        KeyCode::Right => self.state.next_page(),
                        _ => {}
                    }
                }
            }
            KeyCode::Home => {
                if self.state.focus == PaginationFocus::Navigation {
                    self.state.first_page();
                }
            }
            KeyCode::End => {
                if self.state.focus == PaginationFocus::Navigation {
                    self.state.last_page();
                }
            }
            KeyCode::Char(ch) => {
                // Handle text input for range values
                match self.state.focus {
                    PaginationFocus::RangeStart => {
                        self.state.range_start.push(ch);
                    }
                    PaginationFocus::RangeEnd => {
                        self.state.range_end.push(ch);
                    }
                    _ => {}
                }
            }
            KeyCode::Backspace => {
                // Handle backspace for range values
                match self.state.focus {
                    PaginationFocus::RangeStart => {
                        self.state.range_start.pop();
                    }
                    PaginationFocus::RangeEnd => {
                        self.state.range_end.pop();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        vec![]
    }
}
