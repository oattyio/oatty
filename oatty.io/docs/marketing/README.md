# Oatty marketing

This directory defines Oatty's marketing strategy. Read the documents in order. Later documents add research and plans; they don't change the audience, promise, or claim rules set earlier.

Status: active

Effective date: October 13, 2026

Messaging revision: October 18, 2026

Owner: Product

Review cadence: monthly, and before a material homepage, README, package, launch, or campaign change

## Current decision

The campaign lead is **AI is changing how you build. You still own the system.** The product promise is **Understand and operate your stack with Oatty.** Explain what Oatty does, then give the engineer a question to try on their own environment. Show the commands and values behind the answer.

The three entry questions cover understanding an environment, investigating a problem, and assessing a change. Show setup, configuration, and checks as well as debugging. One successful API call measures onboarding. Activation is a verified useful task spanning at least two vendors: an answer checked against retrieved evidence or an operation checked against external state. Don't make claims about the engineer's employability.

Explain why someone would return: the configured catalog stays available, more services can be added, and useful HTTP operations can be saved as workflows. Check current results on each task; the catalog doesn't monitor or remember the environment. Use an internal API without a published schema as one example of agent-assisted setup.

Free and open source helps users try, inspect, and extend Oatty. We still need to test demand, preference over alternatives, and repeat use. The agent supplies the reasoning. Document 03 sets the product claim limits.

Also test Oatty with platform and data-platform engineers: try an API, check the integration, and use what they learn in code and tests. The agent and engineer write those files in their development tools. Deployment, orchestration, and bulk data movement stay with the dedicated runtimes. Oatty doesn't generate or export the implementation itself.

This revision updates the marketing series, root README, homepage, social metadata, and package description locally. It doesn't establish publication or verify onboarding on a released build. Historical research snapshots keep their original dates.

Use the [voice, tone, and review checklist](02-messaging-and-voice.md#voice) for future edits. Read copy aloud: could an engineer say it to a colleague without wincing?

## Canonical series

1. [Strategy and positioning](01-strategy-and-positioning.md) defines the audience, job, category, promise, strategic choices, and adoption model.
2. [Messaging and voice](02-messaging-and-voice.md) translates the strategy into public language and writing rules.
3. [Product truth and claims](03-product-truth-and-claims.md) states what Oatty can prove, what needs qualification, and what must not be published.
4. [Market and competitive landscape](04-market-and-competitive-landscape.md) describes alternatives, differentiation hypotheses, risks, and current market evidence.
5. [Acquisition and measurement](05-acquisition-and-measurement.md) defines the website, content, distribution, funnel, and metrics.
6. [Validation plan](06-validation-plan.md) defines capability audits, customer research, comparative tests, and decision criteria.

The [evidence directory](evidence/) contains dated research snapshots. Keep those dates and sources intact. Recheck time-sensitive facts before using them again; snapshots don't override the strategy.

## Decision states

- **Approved**: use this direction consistently across public surfaces.
- **Verified**: supported by inspected implementation, a reproducible test, or measured data with a recorded date and version.
- **Documented**: described by a primary source, but not independently exercised by Oatty's maintainers.
- **Hypothesis**: plausible and worth testing, but not established as user demand, differentiation, or performance.
- **Blocked claim**: do not publish without the evidence specified in [product truth and claims](03-product-truth-and-claims.md).

## Precedence and updates

The homepage and root README follow this strategy. If they disagree with it, resolve the difference explicitly. Don't change the strategy merely to justify existing copy.

Every material update must:

1. Identify the decision or new evidence.
2. Update the one canonical document that owns the subject.
3. Update dependent public copy and links.
4. Record the validation date for time-sensitive claims.
5. Remove superseded language instead of adding another parallel strategy file.

Keep product documentation, integration runbooks, and workflow examples outside this directory. They should follow these message and claim rules.
