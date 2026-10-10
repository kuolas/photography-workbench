# Security Policy

## Reporting a vulnerability

Please report suspected vulnerabilities **privately**, with *Report a
vulnerability* on the repository's
[Security tab](https://github.com/kuolas/photography-workbench/security).
Only you and the maintainer can see the report. Don't open a public issue,
pull request or comment for anything security-sensitive: the tracker is
public, and a report there discloses the problem before it can be fixed.

Include what you can: the affected component, steps to reproduce or a proof of
concept, and the impact as you understand it.

You will get an acknowledgement within **7 days**. Please allow a reasonable
window for a fix before any public disclosure; you will be credited in the
release notes unless you prefer otherwise.

Bugs that are not security-sensitive go in the public
[issue tracker](https://github.com/kuolas/photography-workbench/issues).

## Supported versions

Nothing has been released yet. Once something is, only the latest release
will receive security fixes.

## Scope

Nothing in the program reads untrusted input yet — it prints a greeting. A
photo culling app will read image files and their metadata, which are
untrusted input; once it does, crashes and memory-safety problems on malformed
files will be the most valuable reports.
