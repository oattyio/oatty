# Oatty Documentation Voice & Tone Guidelines (v2.1)

## Primary Emotional Outcome

After reading Oatty documentation, the developer should think:  
**“This is powerful.”**

Power is demonstrated through clarity, structure, speed, and immediate execution value. TUI is central to this. CLI is
secondary but necessary for automation.

## Purpose

Oatty documentation exists to accelerate confident execution.

Our audience — developers and operators — must be able to:

- Understand the system quickly
- Execute workflows correctly
- Recover from errors independently
- Recognize architectural leverage immediately

Documentation is a product surface. It reduces the support load, shortens time-to-value, and increases adoption.

### Success Metrics

- Median time-to-first-success < 10 minutes
- ≥ 70% task completion without rereading
- ≥ 4.5/5 helpfulness rating
- Reduced support tickets for documented workflows
- High retention of CLI/TUI workflows after first use

Last updated: 2026-02-11  
Maintained by: Oatty Documentation Team  
Feedback: Open an issue in the docs repository.

---

## Core Voice Principles

Verbiage must be clear, approachable, and concise. Avoid jargon and technical terms unless necessary. Content must not
give the appearance of being written by an LLM. Avoid formal, buzzword-like terms, punctuation, and styles commonly
associated with generated
text.

### 1. Demonstrate Power Through Structure

Oatty is schema-driven and workflow-native. Documentation must reflect that.

**Do:**

- Show system-level outcomes, not isolated commands
- Emphasize reuse across TUI, workflows, CLI and MCP tooling
- Reinforce architectural consistency

**Example**  
Weak: "Import an OpenAPI file to generate commands."  
Strong: "Import an OpenAPI file. Oatty generates a consistent command surface reused by CLI, TUI, workflows, and MCP
tooling."

### 2. Action-First, Outcome-Driven

Start with execution. Minimize narrative build-up.

**Do:**

- Use imperative mood
- Lead with the action
- State the outcome clearly

Good:  
Run `oatty --help` to verify your environment.

Avoid:  
~~You might want to check the help command.~~

### 3. Progressive & Tiered (Content Tiering Model)

Layer information deliberately. Never mix tiers.

- **Tier 1 — Critical Path**  
  Required to complete the core task. Minimal, direct, executable.
- **Tier 2 — Common Variations**  
  Edge cases affecting ≥10% of users. Practical alternatives & fallbacks.
- **Tier 3 — Deep Reference**  
  Internals, architecture, extensibility.

Use **Advanced Note** callouts for Tier 2 and Tier 3 content.

### 4. TUI-First Philosophy

Oatty is structured around guided correctness.

- TUI = discovery + structured interaction
- CLI = automation + scripting + CI
- Workflows = repeatable orchestration

**Do:**

- Lead with TUI path on interactive topics
- Include **CLI Fallback** when applicable
- Reinforce the mental model consistently

### 5. Honest and Precise

Trust is built through clarity.

**Do:**

- State limitations explicitly: "Known limitation: step-level resume/re-run is not yet first-class."
- Document tradeoffs: "npm for fastest setup. Source build remains available for Rust-first workflows."
- Provide realistic time estimates: "Estimated time: 10–15 minutes"

**Don't:**

- Oversell features
- Hide rough edges
- Promise unscheduled future work

### 6. Empathetic and User-Responsive

Respect user competence. Acknowledge friction without blame.

**Do:**

- Surface common failure modes proactively
- Offer clear recovery paths

Good:  
No catalog found. Import one with `oatty import <path-or-url> --kind catalog`.

Avoid:  
~~You probably forgot to import a catalog.~~  
~~Don't worry! It's super easy!~~

---

## Tone Standards

- **Confident** — Authority through clarity, not superiority
- **Precise** — No filler words
- **Structured** — Information organized logically
- **Respectful** — Assume technical competence
- **Neutral** — No idioms, hype, regional metaphors, or casual emojis

Power is expressed through discipline.

---

## Writing Mechanics

- **Person**: Second person ("you") for procedures; third person for reference
- **Voice**: Active preferred; passive only when clearly better
- **Tense**: Present for current behavior; future for outcomes
- **Sentence Length**:
    - Procedural steps: ≤12 words
    - General prose: 15–20 words average
- **Contractions**: Allowed when natural ("you're", "it's")
- **Accessibility**: Provide meaningful alt text for screenshots; use semantic headings; ensure high-contrast code
  blocks (WCAG 2.1 AA basics)

**Avoid** Formal, impersonal tone with high noun/determiner use, low adverbs/adjectives; neutral and objective lacking
emotion.

**Avoid** bullet-point lists, title-case headings, and repetitive sentence lengths; avoid simple copulas like "is/are"
for verbose alternatives.


---

## Page Structure Standard

Recommended order:

1. **Summary** — one sentence goal
2. **Prerequisites** — list only when required (see rule below)
3. **Learn Bullets** — 3–5 key takeaways
4. **Estimated Time** — realistic range
5. Action-oriented sections (TUI-first when applicable)
6. **CLI Fallback** — when relevant
7. **Advanced Note** — for Tier 2/3 content
8. **Feedback prompt**: "Was this page helpful? Rate it or suggest improvements → [link]"

### Prerequisites Rule

Include a **Prerequisites** section when the page requires:

- A non-default environment
- A specific catalog imported
- Authentication state
- Required permissions
- A prior workflow completed

Never assume environment state implicitly.

### Command Reference Template

All command reference pages must include:

1. Purpose (1 sentence)
2. Syntax
3. Required arguments
4. Optional flags
5. Examples (most common first)
6. Error cases & recovery
7. Related commands

### Error Message Writing Guidelines

Follow this structure:

1. State the problem (present tense)
2. State the recovery action
3. Avoid blame

Bad: "Invalid catalog."  
Good: "No catalog found. Import one with `oatty import <path-or-url> --kind catalog`."

---

## Visuals & Screenshots

Screenshots must:

- Show changed state after action
- Indicate cursor/focus location (for TUI)
- Include meaningful alt text
- Be annotated when helpful

Avoid decorative screenshots.

---

## Terminology Governance

- The glossary is canonical
- New domain terms must be added before merging related docs
- Use consistent terminology (always “catalog”, never alternate terms)

Terminology drift weakens perceived system integrity.

---

## Common Pitfalls to Avoid

| Avoid                                 | Use Instead                                |
|---------------------------------------|--------------------------------------------|
| "Simply click…"                       | "Click…"                                   |
| "You can also…"                       | "Alternatively," or "For X use case, use…" |
| "It’s easy to…"                       | State the action without judgment          |
| "Obviously," "Clearly," "As you know" | Remove                                     |
| "Please run the command"              | "Run the command"                          |
| "Note that…" as filler                | Integrate directly or use labeled callouts |
| Assuming prior knowledge without link | "If unfamiliar, see [prereq link]"         |
| Overusing emphasis (bold/italics)     | Reserve for key terms; rely on structure   |

Remove any sentence that does not directly help the user complete the task.

---

## Applying These Guidelines

**When Writing:**

1. Draft for clarity and power
2. Remove ~20% of words (ruthless conciseness)
3. Validate terminology against glossary
4. Confirm Tier 1 content is clean and isolated
5. Ensure CLI fallback exists if relevant

**When Reviewing:**

1. Eliminate passive voice and weak verbs
2. Remove hedging ("might", "could", "seems")
3. Check for hidden prerequisites
4. Validate structure matches templates
5. Confirm tone reflects strength, precision, and respect

**Guiding Standard**  
Every page should answer:

- What does this enable?
- Why is this structurally powerful?
- How quickly can I execute this?

If a reader finishes a page and does not recognize leverage, the page must be improved.

Prioritize user success and perceived system power over personal style.