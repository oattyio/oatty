# Oatty --- OSS Coordination Cost Analysis

## Why Multi-Step API Work Is Mentally Expensive (And How Oatty Reduces It)

> Superseded: use `/Users/justinwilaby/Development/next-gen-cli/oatty.io/docs/Oatty_Coordination_Cost_OSS_v2.md` as the canonical version for current messaging.

------------------------------------------------------------------------

# What Is Coordination Cost?

Coordination cost is the mental and operational overhead required to
complete a task across multiple tools, abstractions, and contexts.

For open source developers, coordination cost shows up as:

-   Context switching between tools
-   Translating data between commands
-   Remembering undocumented glue logic
-   Re-implementing retry and validation patterns
-   Debugging ambiguous failures
-   Managing implicit dependencies

This analysis evaluates coordination cost across common approaches to
multi-step API workflows.

------------------------------------------------------------------------

# Scenario

Deploy and validate a cloud service using:

1.  API calls
2.  Conditional logic
3.  Polling / retries
4.  Validation
5.  Structured failure handling

------------------------------------------------------------------------

# Coordination Cost Model

We evaluate five dimensions:

1.  Surface Area (number of tools required)
2.  Mental Model Count (distinct abstraction systems involved)
3.  Implicit Coupling Risk
4.  Failure Debuggability
5.  Refactor Safety

Scoring:\
1 = Low coordination cost\
5 = High coordination cost

------------------------------------------------------------------------

# Comparative Coordination Cost Matrix

  Dimension                Shell Script   GitHub Actions   MCP-only   Oatty
  ------------------------ -------------- ---------------- ---------- -------
  Surface Area             5              4                4          2
  Mental Model Count       5              4                4          2
  Implicit Coupling Risk   5              4                5          1
  Failure Debuggability    5              4                5          1
  Refactor Safety          5              4                5          2

------------------------------------------------------------------------

# Analysis

## Shell Script

Coordination surfaces include:

-   Bash syntax
-   curl semantics
-   jq parsing
-   Environment variable management
-   Manual retry loops
-   Implicit output contracts

Every dependency is implicit. Failure surfaces are ambiguous. Refactors
are fragile.

Coordination cost: High.

------------------------------------------------------------------------

## GitHub Actions

Adds:

-   CI environment constraints
-   YAML workflow model
-   Remote-only execution
-   Secret injection semantics

Better structure than shell. Still lacks local-first and interactive
resolution.

Coordination cost: Medium-High.

------------------------------------------------------------------------

## MCP-Only Toolchains

Adds:

-   LLM unpredictability
-   Tool schema translation
-   Implicit output mapping
-   Hidden dependency binding

Fast to prototype. Hard to make deterministic.

Coordination cost: High.

------------------------------------------------------------------------

## Oatty

Collapses:

-   Discovery
-   Execution
-   Validation
-   Retry logic
-   Explicit dependency declaration
-   Structured error output

Into a single operational model.

Provider-backed inputs must declare dependencies. Workflows validate
before execution. Polling is declarative. Failures are categorized and
actionable.

Coordination cost: Low.

------------------------------------------------------------------------

# Where Oatty Reduces Real Friction

## 1. Surface Compression

Replaces:

bash + curl + jq + CI YAML + retry logic + implicit mapping

With:

Single schema-driven execution surface.

------------------------------------------------------------------------

## 2. Dependency Explicitness

Explicit `depends_on` eliminates hidden coupling.

Shell: silent runtime error. Oatty: blocked at validation.

------------------------------------------------------------------------

## 3. Fewer Mental Models

Developers learn one execution model that powers:

-   CLI
-   TUI
-   Workflows
-   MCP tools

Instead of juggling different paradigms.

------------------------------------------------------------------------

## 4. Deterministic Orchestration

Removes ambiguity introduced by LLM chaining or implicit scripting.

------------------------------------------------------------------------

# Honest Tradeoffs

-   YAML remains necessary for complex workflows.
-   Resume / rerun semantics are evolving.
-   Ecosystem maturity is lower than CI platforms.

------------------------------------------------------------------------

# Bottom Line for OSS

Oatty does not just shorten workflows.

It reduces the number of mental models required to operate APIs.

For open source developers, that means:

-   Less glue code
-   Fewer hidden traps
-   Faster iteration
-   More confidence in automation

Coordination cost reduction is Oatty's true leverage.
