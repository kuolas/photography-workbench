# ADR006: AI-assisted contributions

- Status: Accepted
- Date: 2026-10-10

## Context

Contributors increasingly write code, documentation and reports with the help
of AI tools. Open-source projects have answered in three ways: an outright ban
(Gentoo, NetBSD, QEMU, and within GNOME, Loupe and GNOME Calendar), no policy
at all, or allowing the tools while keeping a human accountable (Fedora,
LLVM). A ban cannot be enforced and would forbid tools the maintainer uses
too; no policy leaves maintainers to absorb unreviewed output.

## Decision

We will adopt the
[Fedora AI-Assisted Contributions Policy](https://docs.fedoraproject.org/en-US/council/policy/ai-contribution-policy/)
(Fedora Council, version 1.0, last reviewed 2025-10-24), restated here in our
own words with only the project-specific parts changed. The key words MAY,
MUST, MUST NOT and SHOULD are used as described in
[RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

You MAY use AI assistance to contribute to photography-workbench, following
these principles:

- **Accountability.** You MUST take responsibility for your contribution: by
  submitting it you vouch for its quality, its license compliance (it must be
  contributable under GPL-3.0-or-later) and its usefulness. Every
  contribution, whether written by a person or with the help of an LLM or
  other generative AI tool, must meet the project's standards. The
  contributor is always the author and is fully accountable for all of it.
- **Transparency.** You MUST disclose the use of AI tools when a significant
  part of the contribution is taken from a tool without changes. You SHOULD
  disclose other uses where it might be useful. Routine help with grammar,
  spelling or clarity needs no disclosure. Disclosure goes where authorship
  is normally recorded: for commits, an `Assisted-by:` trailer
  (`Assisted-by: generic LLM chatbot`, `Assisted-by: Claude`); for issues,
  comments and other contributions outside git, a short note in the text.
- **Contribution and community evaluation.** AI tools MAY assist reviewers
  with analysis and suggestions. AI MUST NOT be the sole or final arbiter of
  a substantive or subjective judgment on a contribution, nor be used to
  evaluate a person's standing in the community (for example roles or Code
  of Conduct matters). Automated objective checks — CI, tests, spam
  filtering — are not affected. Final accountability for accepting a
  contribution, even through an automated system, rests with the person who
  authorizes it.
- **Large-scale initiatives** that could significantly change how the
  project works, or cause a sudden growth in contributions (for example bots
  or agents opening pull requests), are outside this policy and MUST be
  discussed with the maintainers in an issue first.

Concerns about possible violations are reported privately to the maintainer
at the address in [CODE_OF_CONDUCT.md](../../CODE_OF_CONDUCT.md), not in a
public issue.

`Co-Authored-By:` naming a tool remains refused: the contributor is the
author, and `Assisted-by:` is the disclosure.

## Consequences

Contributors can use AI tools openly, and disclosure tells reviewers where to
look harder and helps the project learn how the tools are used. The policy
relies on honesty — undisclosed use cannot be detected reliably — so review
quality remains the real safeguard; maintainers may decline contributions the
submitter cannot explain.

If Fedora revises its policy, this ADR is not edited in place; a new ADR
decides whether to follow.
