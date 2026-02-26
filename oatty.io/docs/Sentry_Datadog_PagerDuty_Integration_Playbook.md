# Sentry + Datadog + PagerDuty Integration Playbook

## Purpose

This runbook documents the current integration style and a repeatable procedure to configure, verify, and roll back:

- Sentry -> PagerDuty
- Datadog -> PagerDuty
- Sentry -> Datadog status check (provider availability)

Use this when onboarding a new environment or re-running integration setup.

## Current Integration Style

- Sentry <-> PagerDuty: native Sentry PagerDuty integration actions.
- Datadog <-> PagerDuty: service-key mapping via Datadog PagerDuty integration.
- Sentry <-> Datadog: no native provider available in this org at time of verification.

## Preflight

1. Confirm Oatty provider auth is valid for `sentry`, `datadog`, `pagerduty`.
2. Confirm target org/service identifiers:
   - Sentry org slug (example: `oattyio`)
   - PagerDuty service name (example: `Default Service`)
3. Never store PagerDuty integration keys in git.

## Step 1: Confirm Sentry Integration Provider Availability

Run:

```text
sentry organizations:config:integrations:list <ORG_SLUG>
```

Expected:

- `pagerduty` provider exists.
- `datadog` provider may not exist.

Optional targeted check:

```text
sentry organizations:config:integrations:list <ORG_SLUG> --providerKey datadog
```

If this returns provider not found, treat Sentry->Datadog native install as unavailable and use alternate forwarding patterns.

## Step 2: Confirm PagerDuty Service and Events API v2 Integration

List PagerDuty services:

```text
pagerduty services:list
```

Fetch integrations for target service:

```text
pagerduty services:info <SERVICE_ID> --include[]=integrations
```

Get Events API v2 integration details (use the integration ID from previous output):

```text
pagerduty services:integrations:info <SERVICE_ID> <INTEGRATION_ID>
```

Capture:

- `service_name`
- `integration_key` (secret)

## Step 3: Configure Datadog -> PagerDuty Mapping

Create mapping in Datadog:

```text
datadog api:integration:pagerduty:configuration:services:create \
  --service_name "<PAGERDUTY_SERVICE_NAME>" \
  --service_key "<PAGERDUTY_EVENTS_API_V2_INTEGRATION_KEY>"
```

Verify mapping:

```text
datadog api:integration:pagerduty:configuration:services:info "<PAGERDUTY_SERVICE_NAME>"
```

Expected: response contains the target `service_name`.

## Step 4: Validate Datadog Notification Handle (No Paging)

Validate monitor payload (safe, no monitor created):

```text
datadog api:monitor:validate:create \
  --type "query alert" \
  --query "avg(last_5m):avg:system.load.1{*} > 100000" \
  --name "integration-validation-pd-handle" \
  --message "Datadog->PagerDuty validation @pagerduty-<SERVICE-NAME-HANDLE>" \
  --options '{"notify_no_data":false,"thresholds":{"critical":100000}}'
```

Recommended handle format:

- `@pagerduty-Default-Service`

## Step 5: Create a Draft Datadog Validation Monitor

Create in draft mode to avoid notifications:

```text
datadog api:monitor:create \
  --name "integration-validation datadog-to-pagerduty" \
  --type "query alert" \
  --query "avg(last_5m):avg:system.load.1{*} > 100000" \
  --message "Datadog to PagerDuty validation @pagerduty-Default-Service" \
  --tags '["integration:datadog-pagerduty","env:production","managed-by:oatty"]' \
  --priority 3 \
  --draft_status draft \
  --options '{"notify_no_data":false,"include_tags":true,"thresholds":{"critical":100000}}'
```

Verify:

```text
datadog api:monitor:info <MONITOR_ID>
```

Expected:

- `draft_status` is `draft`.
- message includes PagerDuty handle.

## Step 6: Sentry -> PagerDuty Validation

Verify at least one Sentry alert rule/workflow includes PagerDuty action:

```text
sentry organizations:alert-rules:list <ORG_SLUG>
```

And/or:

```text
sentry organizations:workflows:list <ORG_SLUG> --project [<PROJECT_ID>]
```

Expected:

- action type `pagerduty`
- non-null integration identifier

## Rollback

Datadog PagerDuty mapping rollback:

```text
datadog api:integration:pagerduty:configuration:services:delete "<PAGERDUTY_SERVICE_NAME>"
```

Datadog validation monitor rollback:

```text
datadog api:monitor:delete <MONITOR_ID>
```

Sentry PagerDuty rollback:

- Remove PagerDuty actions from rules/workflows in Sentry, or disable impacted workflows.

## Operational Notes

- Keep validation monitors in `draft` unless running controlled incident tests.
- Rotate PagerDuty integration keys periodically and update Datadog mapping.
- Prefer service names without ambiguity to avoid routing mistakes.
- Re-run this playbook after org migrations, service renames, or key rotation.

## Re-run Checklist

1. Provider auth confirmed.
2. PagerDuty service and integration key confirmed.
3. Datadog mapping created or updated.
4. Datadog monitor payload validates.
5. Draft monitor exists and is query-valid.
6. Sentry rules/workflows include PagerDuty actions.
7. Rollback commands tested in non-production first.
