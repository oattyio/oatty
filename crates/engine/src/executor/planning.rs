//! Workflow step ordering and dependency planning.

use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::{Result, bail};
use indexmap::IndexMap;

use crate::model::StepSpec;

/// Build a topologically ordered step list for execution.
///
/// Returns an error for duplicate step identifiers, unknown dependencies,
/// self-dependencies, or cycles.
pub fn order_steps_for_execution(steps: &[StepSpec]) -> Result<Vec<&StepSpec>> {
    let mut lookup: IndexMap<String, &StepSpec> = IndexMap::new();
    for step in steps {
        if lookup.contains_key(&step.id) {
            bail!("duplicate step identifier detected: '{}'", step.id);
        }
        lookup.insert(step.id.clone(), step);
    }

    let mut in_degrees: HashMap<String, usize> = lookup.keys().map(|step_id| (step_id.clone(), 0)).collect();
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();

    for (step_id, step) in &lookup {
        let mut seen_dependencies = HashSet::new();
        for dependency in &step.depends_on {
            if !lookup.contains_key(dependency) {
                bail!("step '{}' depends on unknown step '{}'", step_id, dependency);
            }
            if dependency == step_id {
                bail!("step '{}' cannot depend on itself", step_id);
            }
            if !seen_dependencies.insert(dependency) {
                continue;
            }
            *in_degrees.get_mut(step_id).expect("in-degree entry exists") += 1;
            adjacency.entry(dependency.clone()).or_default().push(step_id.clone());
        }
    }

    let mut queue: VecDeque<String> = lookup
        .keys()
        .filter(|step_id| in_degrees.get(*step_id).copied().unwrap_or(0) == 0)
        .cloned()
        .collect();

    let mut ordered = Vec::with_capacity(lookup.len());
    while let Some(step_id) = queue.pop_front() {
        ordered.push(step_id.clone());

        if let Some(children) = adjacency.get(&step_id) {
            for child in children {
                let degree = in_degrees.get_mut(child).expect("dependent step should exist in degrees");
                *degree -= 1;
                if *degree == 0 {
                    queue.push_back(child.clone());
                }
            }
        }
    }

    if ordered.len() != lookup.len() {
        let mut remaining: Vec<String> = in_degrees
            .into_iter()
            .filter(|(_, degree)| *degree > 0)
            .map(|(step_id, _)| step_id)
            .collect();
        remaining.sort();
        bail!("cycle detected in workflow steps involving: {}", remaining.join(", "));
    }

    Ok(ordered.into_iter().map(|step_id| lookup[&step_id]).collect())
}

#[cfg(test)]
mod tests {
    use super::order_steps_for_execution;
    use crate::model::{StepSpec, WorkflowSpec};

    #[test]
    fn prepare_plan_respects_dependencies_even_when_declared_out_of_order() {
        let spec = WorkflowSpec {
            workflow: Some("demo".into()),
            name: Some("Demo".into()),
            inputs: Default::default(),
            steps: vec![
                StepSpec {
                    id: "second".into(),
                    depends_on: vec!["first".into()],
                    run: "echo".into(),
                    ..Default::default()
                },
                StepSpec {
                    id: "first".into(),
                    depends_on: vec![],
                    run: "echo".into(),
                    ..Default::default()
                },
            ],
        };

        let steps = order_steps_for_execution(&spec.steps).expect("plan");
        let ordered_ids: Vec<&str> = steps.iter().map(|step| step.id.as_str()).collect();
        assert_eq!(ordered_ids, vec!["first", "second"]);
    }

    #[test]
    fn prepare_plan_errors_on_unknown_dependency() {
        let spec = WorkflowSpec {
            workflow: Some("demo".into()),
            name: Some("Demo".into()),
            inputs: Default::default(),
            steps: vec![StepSpec {
                id: "only".into(),
                depends_on: vec!["missing".into()],
                run: "echo".into(),
                ..Default::default()
            }],
        };

        let error = order_steps_for_execution(&spec.steps).expect_err("should fail");
        assert!(error.to_string().contains("depends on unknown step"), "unexpected error: {error}");
    }

    #[test]
    fn prepare_plan_errors_on_cycle() {
        let spec = WorkflowSpec {
            workflow: Some("demo".into()),
            name: Some("Demo".into()),
            inputs: Default::default(),
            steps: vec![
                StepSpec {
                    id: "first".into(),
                    depends_on: vec!["second".into()],
                    run: "echo".into(),
                    ..Default::default()
                },
                StepSpec {
                    id: "second".into(),
                    depends_on: vec!["first".into()],
                    run: "echo".into(),
                    ..Default::default()
                },
            ],
        };

        let error = order_steps_for_execution(&spec.steps).expect_err("should detect cycle");
        assert!(error.to_string().contains("cycle detected"), "unexpected error: {error}");
    }
}
