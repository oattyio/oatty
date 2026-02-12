# Oatty --- OSS Positioning Competitive Analysis

## Job #3: Compose and Run Deterministic Multi-Step Workflows

------------------------------------------------------------------------

# Why This Matters to OSS Developers

Open source developers don't need enterprise orchestration engines.

They need:

-   A better alternative to brittle shell scripts
-   A way to chain API calls safely
-   Fewer hidden dependencies
-   Less copy‑paste glue code
-   Clear error messages
-   Local-first execution

This document compares how real developers accomplish this today --- and
what changes with Oatty.

------------------------------------------------------------------------

# Scenario

Deploy and validate a cloud service:

1.  Check if service exists
2.  Create service if missing
3.  Poll until healthy
4.  Validate configuration
5.  Fail with structured error if invalid

------------------------------------------------------------------------

# Real-World Developer Comparison

  Dimension                          Shell Script   GitHub Actions   MCP-Only        Oatty
  ---------------------------------- -------------- ---------------- --------------- -------
  Works locally                      Yes            No (CI-first)    Yes             Yes
  Explicit step graph                No             No               No              Yes
  Polling built-in                   No             Partial          No              Yes
  Structured validation before run   No             No               No              Yes
  Clear error codes                  No             No               No              Yes
  Context-aware input resolution     No             No               LLM-dependent   Yes
  Deterministic execution            Manual         CI-bound         LLM-dependent   Yes
  Human confirmation support         No             No               No              Yes

------------------------------------------------------------------------

# Estimated Implementation Complexity

  Task                     Shell          GitHub Actions    Oatty
  ------------------------ -------------- ----------------- -----------------
  Basic workflow           120--180 LOC   80--120 LOC       60--90 LOC
  Add polling + timeout    Manual loop    Limited           Native
  Add structured failure   Manual         Limited           Native
  Refactor later           Risky          Medium friction   Safe & explicit

------------------------------------------------------------------------

# Where Oatty Wins for OSS

## 1. Less Glue Code

Native `repeat`, `depends_on`, and structured outputs eliminate 40--60%
of orchestration boilerplate compared to bash.

## 2. Fewer Hidden Footguns

Hard dependency validation prevents implicit data flow errors before
execution.

Shell silently fails. Oatty blocks invalid runs.

## 3. Cleaner Failure Surfaces

Errors are structured, categorized, and actionable --- not raw stack
traces or log noise.

## 4. Local-First Automation

Unlike GitHub Actions or cloud-native engines, workflows run locally
with full control and visibility.

------------------------------------------------------------------------

# Honest Tradeoffs

-   Not as battle-tested as CI engines.
-   YAML still required for advanced workflows.
-   Step-level resume is evolving.

------------------------------------------------------------------------

# Why This Matters for Open Source

Oatty is not trying to replace Kubernetes or Terraform.

It replaces:

-   200-line bash scripts
-   Repetitive CI YAML glue
-   Ad-hoc API chaining
-   LLM-driven guesswork without guardrails

It gives OSS developers:

-   Deterministic orchestration
-   Schema-aware safety
-   Interactive execution
-   Plugin extensibility

All without leaving the terminal.

------------------------------------------------------------------------

# Bottom Line

If you already write multi-step API scripts in bash, Oatty makes them:

-   Shorter
-   Safer
-   Easier to debug
-   Easier to share

And that's the kind of leverage OSS developers immediately recognize.
