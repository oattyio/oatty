# Oatty Competitive Analysis Framework

## Purpose
This framework analyzes Oatty against incumbent workflows and tool stacks when no single direct competitor maps to the full product. It focuses on outcome quality, coordination cost, and adoption impact.

## Guiding Principle
Do not compare Oatty to one tool category in isolation.  
Compare Oatty to the *real operational stack users run today*.

## Incumbent Stack Categories
Use these as baseline archetypes:
1. Vendor CLI + shell scripts
2. Curl/Postman + ad hoc runbooks
3. Workflow engine + separate API tooling
4. MCP-only toolchains without unified workflow execution

## Core Evaluation Method
Use a **job-based composition matrix** with proof scenarios.

---

## 1) Job Matrix (Before vs After)
Evaluate high-impact jobs:
1. Import API/catalog and discover executable commands
2. Execute command safely with clear feedback
3. Compose and run deterministic multi-step workflows
4. Add provider-backed inputs with dependency controls
5. Run human-in-the-loop confirmation path
6. Reuse workflow in CI/local contexts
7. Extend via MCP/plugin integration

For each job, compare:
1. Current incumbent stack process
2. Oatty process
3. Delta in:
   - Steps
   - Time
   - Error/retry rate
   - Recovery clarity
   - Cross-tool context switching

### Matrix Template
| Job | Incumbent Stack | Oatty | Steps Delta | Time Delta | Recovery Quality Delta | Notes |
|---|---|---|---:|---:|---|---|
| Import + discover |  |  |  |  |  |  |
| Execute safely |  |  |  |  |  |  |
| Compose workflow |  |  |  |  |  |  |
| Provider-backed inputs |  |  |  |  |  |  |
| HITL run |  |  |  |  |  |  |
| CI/local reuse |  |  |  |  |  |  |
| MCP/plugin extension |  |  |  |  |  |  |

---

## 2) Coordination Cost Analysis
Measure the integration penalty of fragmented stacks.

### Coordination Cost Factors
1. Number of tools touched per job
2. Number of handoffs (copy/paste, context translation)
3. Number of glue scripts maintained
4. Number of places where execution semantics drift

### Scoring (1-5, lower is better)
1. Tool sprawl
2. Operational drift risk
3. Cognitive overhead
4. Debugging fragmentation

---

## 3) Proof-of-Value Scenario Harness
Run side-by-side scenarios in incumbent vs Oatty.

### Required Scenarios (Phase 1)
1. Discover + run first command
2. Build + run first workflow
3. Add provider-backed input and run with confirmation
4. Re-run same workflow in CI-style context

### Scenario Capture Template
| Scenario | Stack | Duration | # Steps | # Errors | # Retries | Recovery Time | Observer Notes |
|---|---|---:|---:|---:|---:|---:|---|
| First command | Incumbent |  |  |  |  |  |  |
| First command | Oatty |  |  |  |  |  |  |
| First workflow | Incumbent |  |  |  |  |  |  |
| First workflow | Oatty |  |  |  |  |  |  |

---

## 4) Differentiation Claims (Evidence-Backed)
State claims as composition outcomes, not novelty claims.

Recommended claim format:
1. **What users do today**: fragmented stack behavior.
2. **What Oatty changes**: cohesive execution plane behavior.
3. **Evidence**: measured scenario deltas + user quotes.

Example:
1. Today: users discover commands in docs and execute in separate tools/scripts.
2. Oatty: command discovery, execution, and workflow composition share one model.
3. Evidence: reduced median steps/time across scenario harness.

---

## 5) Win/Loss Criteria
Use criteria tied to adoption outcomes, not feature checkboxes.

### Win Signals
1. User reaches first successful workflow execution.
2. User chooses Oatty for repeated task execution in subsequent sessions.
3. User reports reduced script/glue maintenance burden.

### Loss Signals
1. User returns to fragmented stack after initial trial.
2. User cannot complete first workflow without support.
3. User perceives Oatty value as equivalent to a single incumbent tool.

---

## 6) Decision Output
At the end of each analysis cycle, produce:
1. Top 3 differentiators with evidence.
2. Top 3 objections with mitigation.
3. Recommended doc narrative updates:
   - Quick Start wording
   - Golden Path emphasis
   - Persona-specific guide priorities

---

## 7) Cadence
1. Initial baseline run: before/at docs phase-1 launch.
2. Review cadence: bi-weekly during phase 1, monthly after stabilization.
3. Owner: Product + DX with Engineering SME support.

---

## 8) Evidence Log (Template)
| Date | Persona | Scenario | Incumbent Summary | Oatty Summary | Key Delta | Quote/Observation | Action |
|---|---|---|---|---|---|---|---|
|  |  |  |  |  |  |  |  |
