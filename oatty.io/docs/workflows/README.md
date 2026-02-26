# Integration Workflows

These workflows automate the Sentry, Datadog, and PagerDuty integration checks and setup we established.

## Workflows

- `integration_preflight.yaml`
  - Read-only baseline checks across Sentry, PagerDuty, and Datadog mapping state.
- `datadog_pagerduty_config.yaml`
  - Creates Datadog -> PagerDuty service mapping using a PagerDuty Events API v2 integration key.
- `datadog_pagerduty_validation_monitor.yaml`
  - Creates a draft Datadog validation monitor using a PagerDuty notification handle.
- `sentry_pagerduty_audit.yaml`
  - Read-only Sentry alert/workflow/detector audit for PagerDuty wiring.

## Recommended Run Order

1. `integration_preflight`
2. `datadog_pagerduty_config`
3. `datadog_pagerduty_validation_monitor`
4. `sentry_pagerduty_audit`

## Safety Notes

- Keep validation monitors in `draft` unless intentionally running a paging test.
- Do not commit real PagerDuty integration keys in workflow defaults, examples, or logs.
- `integration_preflight` intentionally avoids requesting full PagerDuty integration details to reduce secret exposure.
