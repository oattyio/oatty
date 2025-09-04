/// Checks if the workflows feature is enabled via environment variable.
///
/// This function checks the `FEATURE_WORKFLOWS` environment variable to
/// determine whether workflow-related functionality should be enabled. The
/// feature is enabled if the variable is set to "1" or "true"
/// (case-insensitive).
///
/// # Returns
///
/// `true` if workflows are enabled, `false` otherwise.
///
/// # Examples
///
/// ```rust
/// use heroku_registry::feat_gate::feature_workflows;
///
/// if feature_workflows() {
///     println!("Workflows are enabled");
/// } else {
///     println!("Workflows are disabled");
/// }
/// ```
pub fn feature_workflows() -> bool {
    std::env::var("FEATURE_WORKFLOWS")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
}
