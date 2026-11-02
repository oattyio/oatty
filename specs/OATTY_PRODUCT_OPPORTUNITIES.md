# Oatty product opportunities

> **Note**: This document references a marketing series in `oatty.io/docs/marketing/` which is a local working copy not included in the repository.

Status: review draft; recommendations, not approved product decisions or as-built behavior

Assessment date: October 13, 2026

Revised: October 14, 2026, following product review

Evidence: repository source and competitor documentation inspected October 13; no comparative runtime testing or new competitor audit in this revision

This assessment proposes features, optimizations, and scope reductions that support Oatty's marketing direction. It does not supersede the [canonical marketing series](../oatty.io/docs/marketing/README.md). Accepted decisions should be reflected in the relevant strategy, claims, competitive landscape, and validation documents, with implementation tracked separately.

## Recommendation

**Oatty is where API fixes and their verification survive schema churn.**

This is the proposed product wedge, not a claim that the current implementation already delivers it. The core asset is a growing repair history: what broke, the correction, the evidence that it worked, and what happened when the provider changed again. Portability, evidence handling, and direct inspection support that asset.

That fits the values in the [positioning document](../oatty.io/docs/marketing/01-strategy-and-positioning.md): local ownership, engineer control, shared execution, and reuse. The investment order is **durable repairs → portable projects → richer evidence and unknown semantics → direct TUI inspection**. This replaces the previous ordering; these are not four equal priorities.

| Priority | Change | Role in the wedge | Completion evidence |
| --- | --- | --- | --- |
| 1 | Catalog repairs that survive schema updates | Creates the asset that accumulates value | Refresh preserves a valid correction, identifies a conflict, or shows that upstream fixed the defect |
| 2 | Portable integration projects | Lets the repair and its verification benefit another engineer | A sanitized project can be rebound and independently verified in another environment |
| 3 | Retained evidence and explicit unknown outcomes | Makes verification trustworthy as schemas and environments change | Missing, stale, conflicting, or removed evidence cannot become a verified result |
| 4 | A shared execution record with direct TUI inspection | Makes the accumulated repair history easier to inspect and maintain | An engineer can inspect a repair, its evidence, and its later verification runs directly |

Every repair needs a minimal verification record from the first increment, and sharing requires sanitization before release. Those are acceptance requirements within priorities 1 and 2. Priority 3 expands the evidence model across services and execution paths; priority 4 adds the richer inspection experience. Neither requires building the full later feature before shipping durable repairs.

## Initial focus: integration development and maintenance

Choose **development first**, specifically engineers making internal, custom, or imperfect vendor APIs usable and keeping those integrations working through upstream changes. The recurring job is to discover a defect, correct the catalog, establish evidence, and maintain the correction over later schema revisions.

Operations is a downstream use of the maintained integration. Existing operational commands remain useful, but incident response, continuous production discovery, and autonomous remediation do not drive this roadmap. This is a proposed narrowing of the initial market entry within "Understand and operate your stack"; accepting it would require updating the canonical audience and adoption documents.

The adoption event to test is a verified repair. The retention event is reuse of that repair during a later refresh, integration task, or colleague's handoff. A one-off successful API call does not establish the wedge.

## Competitive implications

The competitive bar is higher than the current differentiators suggest:

- **Bruno deserves explicit inclusion in the competitive matrix.** Its official MCP server already lets agents discover, inspect, and execute local, Git-versioned collections with tests and environments. "Local + human/agent access + reusable checks" is therefore insufficient differentiation. [Bruno MCP](https://github.com/usebruno/bruno-mcp)
- **Portability and discovery are becoming expected features.** Postman supports local Git projects, and FastMCP supports search-based tool discovery. [Postman Native Git](https://learning.postman.com/docs/use/native-git/overview/), [FastMCP tool search](https://gofastmcp.com/servers/transforms/tool-search)
- **Production understanding and verification also have competition.** Resolve advertises production context and post-deployment checks; Rootly links investigation conclusions to rerunnable queries. [Resolve](https://resolve.ai/blog/resolve-ai-plugin-cursor-claude-code-codex), [Rootly](https://rootly.com/changelog/rootly-ai-gathers-incident-evidence-across-your-stack)

For the development focus, the primary comparisons are Bruno plus the same agent, direct vendor tools with maintained scripts, and an OpenAPI-to-MCP framework. Production-context products remain adjacent alternatives; they do not justify expanding this feature list. Compare maintenance across successive schema revisions, not just first-call setup. The cited sources establish documented or advertised capabilities, not independently measured performance.

## 1. Make catalog repair durable and reviewable

This is the most concrete implementation gap identified in the assessment. Patches persist, but an overwrite import regenerates commands and preserves selected connection settings rather than previous command repairs. See [catalog patch persistence](../crates/registry/src/catalog_patch.rs) and the [import and configuration preservation path](../crates/registry/src/openapi_import.rs).

Add a recorded correction containing:

- The original schema fingerprint, affected operation, and exact command change.
- The observed failure, rationale, and a repeatable verification check.
- The tested schema and command revisions, verification time, and a sanitized evidence reference.
- A comparison when upstream changes arrive, including whether the provider has fixed the original defect.
- Explicit conflicts when the correction cannot be reapplied confidently, with the previous working revision retained for review.

The resulting experience is:

> We fixed this integration once; refreshing its schema shows whether that fix is still needed.

This is the first product investment. It strengthens an existing capability without requiring Oatty to diagnose or repair APIs autonomously. Reapplication alone does not establish correctness: changed applicability or missing verification leaves the repair unverified. A catalog revision rollback does not undo external API side effects.

## 2. Make verified repairs portable

A small project directory should carry catalog references, corrections, verification procedures, and the assumptions required to use them. Its purpose is to transfer a maintained repair, rather than merely export requests.

The acceptance test is practical:

> Another engineer imports a sanitized repair project, binds their own environment and credentials, and verifies the correction without repeating the original investigation.

Git export alone will not distinguish Oatty from Bruno or Postman. Preserving a complete, understandable repair history might. Begin with ordinary files and existing workflows; a marketplace or elaborate bundle system is unnecessary initially.

### Sharing boundary: credentials and topology

Secret references alone are insufficient. Environment names, internal hostnames, tenant and resource IDs, schema URLs, repository paths, private revision identifiers, and relationships between services can expose topology even when credential values are absent. A fingerprint of private material should not automatically be considered safe to publish.

Use an explicit separation between local execution state and the portable artifact:

- **Local state:** actual endpoints, account mappings, credential bindings, private schema provenance, and any permitted retained observations. These stay outside the export set. Retain only the evidence needed for verification and redact before persistence.
- **Portable artifact:** reviewed corrections, parameterized checks, logical names, required access descriptions, synthetic examples, and explicitly approved provenance. Include public provider versions where appropriate; private mappings and revision identifiers require review.
- **Export process:** construct the artifact from an allowlist, replace environment-specific values consistently, scan the output and all included metadata, and show a reviewable manifest/diff before publication. Scanning supports review; it cannot prove that arbitrary prose or identifiers reveal nothing sensitive. Ordinary source, filename, and Git-history copying must not bypass this boundary.
- **Recipient behavior:** require local binding and fresh verification. Do not execute checks or write operations merely by importing a project. Removed evidence makes the transferred result unknown until rerun; it must not retain a misleading green status.

The sharing claim is blocked until a fixture containing seeded credentials and topology identifiers passes the export checks and a second engineer can rebind and verify the sanitized artifact. Private repair records must never become community contributions implicitly.

This extends the [platform-development handoff opportunity](../oatty.io/docs/marketing/01-strategy-and-positioning.md#preserve-understanding-as-well-as-operations). Maintained application code, infrastructure definitions, and orchestration retain their intended owners.

## 3. Retain evidence and make uncertainty explicit

An assertion primitive is table stakes. The product value is preserving why a repair was considered correct and knowing when that evidence no longer supports the conclusion.

Each verification record should identify its predicate, supporting operations and selected values, relevant revisions, environment binding, observation time, and validity conditions. Store sanitized evidence; do not equate evidence retention with retaining raw responses. Give every result one of three meanings:

| Result | Meaning | Consequence |
| --- | --- | --- |
| Pass | Sufficient applicable evidence satisfies the stated predicate | Mark that predicate verified for the recorded scope and time |
| Fail | Sufficient applicable evidence contradicts the predicate | Record the counterexample and leave the repair unverified |
| Unknown | Evidence is missing, inaccessible, stale, conflicting, redacted beyond usefulness, or invalidated by a revision/environment change | Preserve the reason and withhold verified status; do not treat it as authorization to proceed |

Fail closed to **unknown** when the basis for a conclusion is inadequate. Fail and unknown both block any action explicitly gated on successful verification. Neither should trigger an automatic repair or write. Freshness must follow the check's declared policy, not a universal promise that yesterday's pass remains valid.

For a cross-vendor example, verification could require an identity callback to match the hosting domain and the DNS target. The evidence record should show which values establish the relationship and which checks remain unknown. It does not establish that login works end to end unless that outcome is separately tested.

Oatty already has conditional steps and polling through `repeat.until`. The [workflow step model](../crates/types/src/workflow.rs) has no explicit assertion primitive, while the [repeat executor](../crates/engine/src/executor/repeat.rs) already evaluates conditions. Build on those mechanisms. The initial repair record needs only a bounded check and honest status; this later investment generalizes evidence retention across services and CLI/TUI/MCP paths.

## 4. Make repair history directly inspectable

Each repair and verification run should have a stable reference that opens the corresponding operation in the TUI: catalog revision, local environment, sanitized evidence, timing, and linked checks. Show why a patch exists, where it has been verified, and what changed since the last valid result.

Useful actions include comparing runs, inspecting a schema conflict, preparing a corrected invocation, or retiring a patch after an upstream fix. Mutating operations require a fresh execution decision.

Current workflow history contains a run ID and summary, but does not itself provide this complete shared record. See the [history model](../crates/mcp/src/server/workflow/services/history.rs) and [TUI log model](../crates/tui/src/ui/components/logs/state.rs). Keep this focused on repair maintenance and operations passing through Oatty; full agent observability is outside this investment.

## How the advantage could compound

The feature set is replicable. The candidate accumulating asset is a corpus of failure → correction → verification → later revision outcomes. Repeated maintenance can establish which fixes apply to which schema shapes, which checks expose regressions, and which patches can be retired. A new UI or patch format alone does not create this advantage.

There are two distinct accumulation paths:

- **Within a project or team:** private repair history reduces repeated diagnosis and makes handoff easier. This is useful retention value, but it does not automatically create an Oatty-wide moat. Users should remain able to export their work.
- **Across users:** explicitly contributed, sanitized repairs for public APIs could form a maintained compatibility corpus with reproducible fixtures, applicability constraints, reviewer provenance, and verification across revisions. Contributions must pass the same disclosure boundary as project exports. Private integrations must not be mined to populate it.

Competitors can copy public repairs as well as code. Oatty would have to earn an advantage through continuing contributions, trusted maintenance, and accumulated verification coverage. That takes time to reproduce, but its existence and durability are unproven. The defensible claim to test is lower repeated maintenance effort because prior verified work remains useful.

Measure reuse of existing repairs, avoided repeat investigations, verified coverage across schema revisions, conflicting or obsolete patches, and the effort needed to keep the corpus current. Raw patch count is not progress. If repairs are mostly one-off, become stale quickly, or require as much review as recreating them, the compounding thesis fails and the roadmap should be reconsidered.

## Supporting optimizations

### Connection diagnostics and environment profiles

Bind endpoint, account, and credential reference together; expose missing credentials, wrong targets, and insufficient access clearly. Keep those actual bindings local when exporting repairs. Add OAuth flows according to blockers in the selected development tasks. Restish already documents several OAuth flows, so this is adoption work rather than a novel advantage. [Restish OAuth](https://rest.sh/docs/guides/oauth/)

### Bounded HTTP/MCP checks

Saved workflows currently reject MCP-backed steps. Supporting both for verification would close a mismatch in the cross-vendor promise, though tool permissions, output handling, and cancellation need explicit contracts. See the [current runner restriction](../crates/engine/src/executor/runner.rs).

Add this only when selected repair-verification tasks require it. Mixed mutation workflows should be a separate scope decision.

### A few maintained examples spanning services

Ship examples that include a realistic schema defect, its correction, verification, and a later upstream revision. Measure whether users reuse the repair and understand its applicability before expanding vendor coverage.

## Removals and reduced scope

### Remove misleading execution guarantees

`run_safe_command` currently permits downstream MCP commands without enforcing their read-only status. Enforce a defined restriction or revise the name and contract; preserve deliberate write access. See `HttpMethodGuard::ensure_mcp_allowed` in the [MCP server](../crates/mcp/src/server/core.rs).

### Remove raw sensitive inputs from persistent history

Do this before expanding sharing. The workflow execution path currently copies the input map directly into history. See [history construction](../crates/mcp/src/server/workflow/tools/execution.rs) and [history serialization](../crates/mcp/src/server/workflow/services/history.rs).

### Defer broad product expansion

Defer broad orchestration, automatic topology discovery, embedded reasoning, and native infrastructure generation. These would substantially expand Oatty's responsibilities beyond the current positioning.

### Keep interface reduction disciplined

The nine-tool workflow surface is already present in source, so removing the old convenience tools is completed work, not a new roadmap item. See `default_workflow_tool_surface_matches_the_reduced_contract` in the [existing contract tests](../crates/mcp/src/server/core.rs).

Use the [as-built workflow specification](MCP_WORKFLOWS.md) for the current contract.

## Validation and investment order

Follow the stated investment order: **repairs → portable projects → richer evidence semantics → TUI inspection**. Resolve connection blockers for the chosen fixture and history/redaction defects as acceptance requirements for the relevant increment. Do not turn those requirements into a parallel platform roadmap.

Validate the direction with one development integration, such as a domain/hosting/identity setup, across successive schema revisions:

1. Introduce a realistic catalog/schema defect, distinguish it from a service-configuration fault, and record the failing interaction using a controlled fixture.
2. Correct the command definition and independently verify the intended behavior. Retain the minimum sanitized evidence and applicability information.
3. Refresh to a compatible upstream revision. Confirm that the correction remains applicable and rerun verification.
4. Test an incompatible revision and an upstream fix. Require an explicit conflict or patch-retirement decision rather than silently replaying the old correction.
5. Remove permissions, invalidate evidence, and alter the environment binding. Confirm that insufficient evidence yields unknown and cannot pass a verification gate.
6. Export a project seeded with credentials, private endpoints, tenant IDs, revision metadata, and topology descriptions. Inspect every included file and metadata field for disclosure; then have a second engineer bind a different environment and independently verify it.
7. Observe a later maintenance task to determine whether the stored repair history saves work without increasing incorrect conclusions.

Compare against the same agent using Bruno and direct vendor tools with their normal maintained files and tests. Allow alternatives to retain their own fixes and history. Hold the task, access, environment, and required outcome constant. Measure repeat-diagnosis effort, correct conflict handling, false verified results, successful sanitized handoff, and voluntary reuse. Separate initial setup from later maintenance cost; the latter is the central test of the wedge.

This assessment combines current competitor documentation with source inspection. It does not establish runtime superiority, customer demand, or an approved implementation commitment.
