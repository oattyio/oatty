use crate::ui::components::workflows::{JsonSyntaxRole, classify_json_value, format_preview};
use oatty_engine::{ProviderBindingOutcome, WorkflowRunState};
use oatty_types::{WorkflowInputDefinition, WorkflowProviderArgumentValue, WorkflowValueProvider, validate_candidate_value};
use oatty_util::has_meaningful_value;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use ratatui::widgets::ListState;
use std::cell::{Ref, RefCell};
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputStatus {
    Resolved,
    Pending,
    Error,
    Blocked,
}

#[derive(Debug, Clone)]
pub struct WorkflowValuePreview {
    pub text: String,
    pub role: JsonSyntaxRole,
}

impl WorkflowValuePreview {
    fn new(text: String, role: JsonSyntaxRole) -> Self {
        Self { text, role }
    }
}

#[derive(Debug)]
pub struct WorkflowInputRow {
    pub name: String,
    pub required: bool,
    pub provider_label: Option<String>,
    pub status: InputStatus,
    pub status_message: Option<String>,
    pub current_value: Option<WorkflowValuePreview>,
    pub blocked_reason: Option<String>,
}

impl WorkflowInputRow {
    pub fn is_blocked(&self) -> bool {
        self.blocked_reason.is_some()
    }
}
#[derive(Debug)]
pub struct WorkflowInputViewState {
    container_focus: FocusFlag,
    pub mouse_over_idx: Option<usize>,
    pub run_state: Rc<RefCell<WorkflowRunState>>,
    pub input_rows: Vec<WorkflowInputRow>,
    /// list state for the inputs list
    pub input_list_state: ListState,
    /// Focus flag tracking list navigation state.
    pub f_list: FocusFlag,
    /// Focus flag tracking the workflow details pane scroll area.
    pub f_details: FocusFlag,
    /// Focus flag used for the cancel action button.
    pub f_cancel_button: FocusFlag,
    /// Focus flag used for the plan action button.
    pub f_plan_button: FocusFlag,
    /// Focus flag used for the run action button.
    pub f_run_button: FocusFlag,
    details_scroll_offset: u16,
    details_content_height: u16,
    details_viewport_height: u16,
}

impl WorkflowInputViewState {
    pub fn new(run_state: Rc<RefCell<WorkflowRunState>>) -> Self {
        Self {
            mouse_over_idx: None,
            run_state,
            input_rows: Vec::new(),
            input_list_state: ListState::default(),
            container_focus: FocusFlag::new().with_name("workflow.inputs"),
            f_list: FocusFlag::new().with_name("workflow.inputs.list"),
            f_details: FocusFlag::new().with_name("workflow.inputs.details"),
            f_cancel_button: FocusFlag::new().with_name("workflow.inputs.actions.cancel"),
            f_plan_button: FocusFlag::new().with_name("workflow.inputs.actions.plan"),
            f_run_button: FocusFlag::new().with_name("workflow.inputs.actions.run"),
            details_scroll_offset: 0,
            details_content_height: 0,
            details_viewport_height: 0,
        }
    }

    pub fn build_input_rows(&mut self) {
        self.input_rows.clear();
        let run_state = self.run_state.as_ref().borrow();
        for (name, definition) in run_state.workflow.inputs.iter() {
            self.input_rows.push(self.build_input_row(&run_state, name, definition));
        }
    }

    fn build_input_row(&self, run_state: &Ref<WorkflowRunState>, name: &str, definition: &WorkflowInputDefinition) -> WorkflowInputRow {
        let required = definition.is_required();

        let provider_label = definition.provider.as_ref().map(|provider| match provider {
            WorkflowValueProvider::Id(id) => id.clone(),
            WorkflowValueProvider::Detailed(detail) => detail.id.clone(),
        });
        let display_name = definition.display_name(name).into_owned();

        let provider_state = run_state.provider_state_for(name);
        let mut status = InputStatus::Pending;
        let mut status_message = None;

        if let Some(state) = provider_state {
            for outcome_state in state.argument_outcomes.values() {
                match &outcome_state.outcome {
                    ProviderBindingOutcome::Error(error) => {
                        status = InputStatus::Error;
                        status_message = Some(error.message.clone());
                        break;
                    }
                    ProviderBindingOutcome::Prompt(prompt) => {
                        status = InputStatus::Pending;
                        status_message = Some(prompt.reason.message.clone());
                    }
                    ProviderBindingOutcome::Skip(skip) => {
                        status = InputStatus::Pending;
                        status_message = Some(skip.reason.message.clone());
                    }
                    ProviderBindingOutcome::Resolved(_) => {}
                }
            }
        }

        let raw_value = run_state.run_context.inputs.get(name);
        let has_value = raw_value.is_some_and(has_meaningful_value);

        if matches!(status, InputStatus::Error) {
            // Preserve error state and explanatory message coming from provider resolution.
        } else if has_value {
            if let Some(value) = raw_value {
                if let Some(validation) = &definition.validate {
                    match validate_candidate_value(value, validation) {
                        Ok(()) => {
                            status = InputStatus::Resolved;
                            status_message = None;
                        }
                        Err(message) => {
                            status = InputStatus::Error;
                            status_message = Some(format!("{}", message));
                        }
                    }
                } else {
                    status = InputStatus::Resolved;
                    status_message = None;
                }
            }
        } else {
            status = InputStatus::Pending;
        }

        let blocked_reason = dependency_block_reason(run_state, definition);
        if blocked_reason.is_some() && !matches!(status, InputStatus::Error) {
            status = InputStatus::Blocked;
            if status_message.is_none() {
                status_message = blocked_reason.clone();
            }
        }

        let current_value = raw_value.map(|value| WorkflowValuePreview::new(format_preview(value), classify_json_value(value)));
        WorkflowInputRow {
            name: display_name,
            required,
            provider_label,
            status,
            status_message,
            current_value,
            blocked_reason,
        }
    }

    /// Returns the current details pane scroll offset.
    pub fn details_scroll_offset(&self) -> u16 {
        self.details_scroll_offset
    }

    /// Updates the details pane viewport height and clamps scroll state.
    pub fn update_details_viewport_height(&mut self, height: u16) {
        self.details_viewport_height = height;
        self.clamp_details_scroll_offset();
    }

    /// Updates the details pane content height and clamps scroll state.
    pub fn update_details_content_height(&mut self, height: u16) {
        self.details_content_height = height;
        self.clamp_details_scroll_offset();
    }

    /// Scrolls the details pane by line count.
    pub fn scroll_details_lines(&mut self, delta: i16) {
        if delta == 0 {
            return;
        }
        self.apply_details_scroll_delta(delta as i32);
    }

    /// Scrolls the details pane by whole-page increments.
    pub fn scroll_details_pages(&mut self, delta_pages: i16) {
        if delta_pages == 0 || self.details_viewport_height == 0 {
            return;
        }
        let delta = i32::from(self.details_viewport_height).saturating_mul(i32::from(delta_pages));
        self.apply_details_scroll_delta(delta);
    }

    /// Scrolls the details pane to the top.
    pub fn scroll_details_to_top(&mut self) {
        self.details_scroll_offset = 0;
    }

    /// Scrolls the details pane to the bottom.
    pub fn scroll_details_to_bottom(&mut self) {
        self.details_scroll_offset = self.max_details_scroll_offset();
    }

    /// Returns whether the details pane content exceeds its viewport.
    pub fn details_is_scrollable(&self) -> bool {
        self.details_content_height > self.details_viewport_height && self.details_viewport_height > 0
    }

    /// Returns the measured details pane content height.
    pub fn details_content_height(&self) -> u16 {
        self.details_content_height
    }

    /// Returns the measured details pane viewport height.
    pub fn details_viewport_height(&self) -> u16 {
        self.details_viewport_height
    }

    fn apply_details_scroll_delta(&mut self, delta: i32) {
        if delta == 0 || !self.details_is_scrollable() {
            return;
        }
        let current = i32::from(self.details_scroll_offset);
        let max = i32::from(self.max_details_scroll_offset());
        let next = (current + delta).clamp(0, max);
        self.details_scroll_offset = next as u16;
    }

    fn clamp_details_scroll_offset(&mut self) {
        self.details_scroll_offset = self.details_scroll_offset.min(self.max_details_scroll_offset());
    }

    fn max_details_scroll_offset(&self) -> u16 {
        self.details_content_height.saturating_sub(self.details_viewport_height)
    }
}

fn dependency_block_reason(run_state: &WorkflowRunState, definition: &WorkflowInputDefinition) -> Option<String> {
    if definition.depends_on.is_empty() {
        return None;
    }

    for value in definition.depends_on.values() {
        match value {
            WorkflowProviderArgumentValue::Binding(binding) => {
                if let Some(input_name) = binding.from_input.as_deref()
                    && !run_state.run_context.inputs.get(input_name).is_some_and(has_meaningful_value)
                {
                    let label = friendly_input_label(run_state, input_name);
                    return Some(format!("Waiting on input '{label}'"));
                }
                if let Some(step_id) = binding.from_step.as_deref()
                    && !run_state.run_context.steps.get(step_id).is_some_and(has_meaningful_value)
                {
                    return Some(format!("Waiting on step '{step_id}'"));
                }
            }
            WorkflowProviderArgumentValue::Literal(template) => {
                for input_name in extract_template_inputs(template) {
                    if !run_state.run_context.inputs.get(&input_name).is_some_and(has_meaningful_value) {
                        let label = friendly_input_label(run_state, &input_name);
                        return Some(format!("Waiting on input '{label}'"));
                    }
                }
                for step_id in extract_template_steps(template) {
                    if !run_state.run_context.steps.get(&step_id).is_some_and(has_meaningful_value) {
                        return Some(format!("Waiting on step '{step_id}'"));
                    }
                }
            }
        }
    }

    None
}
fn friendly_input_label(run_state: &WorkflowRunState, identifier: &str) -> String {
    run_state
        .workflow
        .inputs
        .get(identifier)
        .map(|definition| definition.display_name(identifier).into_owned())
        .unwrap_or_else(|| identifier.to_string())
}

fn extract_template_inputs(template: &str) -> Vec<String> {
    extract_template_identifiers(template, "inputs.")
}

fn extract_template_steps(template: &str) -> Vec<String> {
    extract_template_identifiers(template, "steps.")
}

fn extract_template_identifiers(template: &str, prefix: &str) -> Vec<String> {
    let mut results: Vec<String> = Vec::new();
    let mut remaining = template;
    while let Some(start) = remaining.find("${{") {
        let after = &remaining[start + 3..];
        if let Some(end) = after.find("}}") {
            let expression = after[..end].trim();
            if let Some(rest) = expression.strip_prefix(prefix)
                && let Some(identifier) = parse_identifier(rest)
                && !results.contains(&identifier)
            {
                results.push(identifier.clone());
            }
            remaining = &after[end + 2..];
        } else {
            break;
        }
    }
    results
}

fn parse_identifier(fragment: &str) -> Option<String> {
    let mut identifier = String::new();
    for ch in fragment.chars() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            identifier.push(ch);
        } else {
            break;
        }
    }
    if identifier.is_empty() { None } else { Some(identifier) }
}

impl HasFocus for WorkflowInputViewState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.leaf_widget(&self.f_list);
        builder.leaf_widget(&self.f_details);
        builder.leaf_widget(&self.f_cancel_button);
        builder.leaf_widget(&self.f_plan_button);
        builder.leaf_widget(&self.f_run_button);
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
