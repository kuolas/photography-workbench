# ADR001: Record architecture decisions

- Status: Accepted
- Date: 2026-10-10

## Context

Decisions about licensing, workflow and design are made once and then live on
in the code and the process. Without a record, the reasons are lost, newcomers
cannot tell a deliberate choice from an accident, and settled debates come
back.

## Decision

We will keep Architecture Decision Records in `docs/architecture-decisions/`,
one Markdown file per decision, numbered `adrNNN-short-name.md`, in the
Context / Decision / Consequences format. The folder's
[README](README.md) indexes them and explains how to add one.

## Consequences

Anyone can find why something is the way it is, and propose a change by
writing a new ADR in a pull request. Records cost a few minutes each and only
help if people keep writing them; a decision taken without one leaves a gap.
