# Oatty Product Requirements Document

## Version 3.0 --- Board & Executive Ready

------------------------------------------------------------------------

# 1. Executive Summary

Oatty introduces a new operational paradigm:

A unified execution surface for APIs that combines schema-derived
commands, an interactive TUI, deterministic workflows, and MCP
extensibility.

This is not a CLI replacement. This is infrastructure for operational
standardization.

The purpose of this initiative is to:

• Reduce API fragmentation\
• Standardize execution patterns\
• Shorten onboarding time\
• Replace brittle scripts with governed workflows\
• Establish a consistent automation surface

Documentation is a core adoption engine --- not a support artifact.

------------------------------------------------------------------------

# 2. Market Context

## The Current State

Vendor tooling is fragmented:

-   Thin CLIs with partial API coverage
-   Separate MCP surfaces with restricted capability
-   Script-based automation lacking validation and review
-   Inconsistent UX patterns across vendors

This produces: - Relearning costs - Operational drift - Cognitive
overload - Platform inefficiencies

## Strategic Opportunity

Developers are increasingly operating across:

-   Multiple SaaS vendors
-   Multiple internal APIs
-   AI-assisted workflows

There is no unified operational layer.

Oatty positions itself as that layer.

------------------------------------------------------------------------

# 3. Positioning Statement

Oatty is the execution plane for API operations.

It transforms APIs from documentation artifacts into discoverable,
executable, and automatable systems.

------------------------------------------------------------------------

# 4. Product Vision

Long-Term Vision:

Oatty becomes the default operational surface for:

-   Vendor APIs
-   Internal platform APIs
-   MCP-based tools
-   Human-in-the-loop automation

The CLI becomes a programmable interface. The TUI becomes an exploratory
surface. Workflows become the automation primitive. MCP becomes the
extension layer.

------------------------------------------------------------------------

# 5. Adoption Funnel Model

Stage 1: Curiosity\
User runs TUI and explores search.

Stage 2: Activation\
User executes first command.

Stage 3: Value Realization\
User creates or runs first workflow.

Stage 4: Commitment\
User replaces existing script with workflow.

Stage 5: Advocacy\
User shares workflow or builds plugin.

------------------------------------------------------------------------

# 6. Activation Definition

First Success (within 10 minutes):

1.  Import an OpenAPI spec
2.  Discover a command via search
3.  Execute the command
4.  Run or create a simple workflow

Instrumentation (future): - Track time-to-first-command - Track
time-to-first-workflow - Track workflow persistence rate

------------------------------------------------------------------------

# 7. Core UX Principles

Discoverability\
Simplicity\
Speed\
Consistency

These are non-negotiable design constraints.

------------------------------------------------------------------------

# 8. Golden Path Experience

Canonical narrative:

1.  Import API
2.  Search command
3.  Execute command
4.  Compose workflow
5.  Add provider
6.  Run workflow with confirmation

This single path powers: - Demo - Website - Quickstart - Conference
talks - Blog posts

------------------------------------------------------------------------

# 9. Differentiation Matrix

  Dimension                  Vendor CLI   Raw MCP   Oatty
  -------------------------- ------------ --------- -------
  Full API Coverage          Partial      Partial   Yes
  Discoverable UI            Limited      None      Yes
  Workflow Native            Rare         No        Yes
  Cross-Vendor Consistency   No           No        Yes
  Human-in-the-Loop          No           Limited   Yes

------------------------------------------------------------------------

# 10. Roadmap Phasing

Phase 1 --- Foundation - Schema-derived commands - TUI search - Workflow
execution

Phase 2 --- Standardization - ValueProviders - Workflow editing UX - MCP
plugin management

Phase 3 --- Expansion - Advanced workflow composition - Enterprise
governance controls - Team workflow sharing

------------------------------------------------------------------------

# 11. Risk Assessment

Risk: Over-Abstract Positioning\
Mitigation: Lead with tangible demos.

Risk: AI Overreach\
Mitigation: Position NL as enhancement, not dependency.

Risk: Cognitive Overload\
Mitigation: Progressive disclosure and Golden Path emphasis.

------------------------------------------------------------------------

# 12. Success Metrics

Short-Term: - Time-to-first-workflow under 10 minutes - 50% of new users
execute workflow in first session

Mid-Term: - 30% reduction in bespoke script usage (internal
environments) - 25% repeat usage rate within 7 days

Long-Term: - Community plugin ecosystem growth - Standardized internal
adoption

------------------------------------------------------------------------

# 13. Organizational Implications

To succeed, Oatty requires:

-   UX discipline enforcement
-   Clear narrative ownership
-   Documentation treated as product
-   Focus on one polished Golden Path before expanding scope

------------------------------------------------------------------------

# 14. Strategic Summary

Oatty is not another CLI.

It is a unification layer.

Its success depends not only on engineering quality, but on clarity of
narrative and speed of user activation.

This PRD establishes the foundation for disciplined execution.
