# Contributing to photography-workbench

Thanks for your interest! photography-workbench is a desktop app for culling
photos, written in Rust. It is at an early stage — one of its three modes
is built, and the scope is still settling — so ideas and bug reports are the
help it needs most. Changes to the code and docs come from contributors; see
[Becoming a contributor](#becoming-a-contributor).

This page is the one place the project's rules live. It is short on purpose;
if something here is unclear, that is a bug in this page, and an issue about it
is a fine first contribution.

Everyone taking part is expected to follow the
[Code of Conduct](CODE_OF_CONDUCT.md).

## Getting started

You need:

- a stable Rust toolchain, **Rust 1.95 or later** (the minimum egui
  supports), with the `rustfmt` and `clippy` components
  (`rustup component add rustfmt clippy`);
- `git`. The GitHub CLI (`gh`) is handy but optional.

Then:

```sh
git clone https://github.com/kuolas/photography-workbench.git
cd photography-workbench
cargo build
cargo run
cargo test
```

To run a single test, or every test whose name contains a word:
`cargo test <name>`. The code is a workspace of crates under `crates/`;
`cargo test -p <crate>` tests one of them.

## Reporting bugs and requesting features

The tracker is [GitHub Issues](https://github.com/kuolas/photography-workbench/issues).
It is the project's one list of what is broken, deferred or wanted.

- **A bug** — use the *Bug report* form: what happened, how to reproduce it,
  and what you expected instead.
- **A feature** — use the *Feature request* form, and start from the problem
  rather than the solution. While the scope is open
  ([docs/ROADMAP.md](docs/ROADMAP.md)), a request is input to that decision
  rather than a place in a queue.
- **Anything else** (a docs fix, a question) — a blank issue is fine.

For anything bigger than a small fix, please open or find an issue **before**
you start, so we can agree on the approach before you spend time on it.

### Labels

The maintainer labels each new issue, so there is nothing for you to pick
when you open one. They sort the tracker by what an issue is, where it
belongs, what kind of work it needs and whether it can start; an issue
often has several. A crate added under `crates/` gets a label of its own
when it lands.

What it is:

- `bug` — something doesn't work, or gives a wrong result. The *Bug report*
  form adds it.
- `enhancement` — a new feature or a change in behaviour. The *Feature
  request* form adds it.

Where it belongs, by the crate it touches:

- `photography-workbench` — the app: the window, its views and its menus
  (`crates/photography-workbench`).
- `desktop-ui` — the custom widgets and the shared parts of the window
  (`crates/desktop-ui`).
- `catalog` — the photos on disk: the folder tree and a folder's photo list
  (`crates/catalog`).
- `previews` — decoding the photos' embedded previews, off the UI thread
  (`crates/previews`).
- `ci` — the CI workflows, the git hooks and the scripts in `scripts/`.
- `docs` — documentation: `docs/`, the READMEs and the API docs.

What kind of work it is:

- `security` — hardening against hostile files, or a vulnerability once it
  has been disclosed. Report a vulnerability privately first, as
  [Security issues](#security-issues) says.
- `performance` — speed, memory use or allocations.
- `testing` — tests, fixtures and test harnesses.
- `accessibility` — a barrier for people with disabilities.
- `platform: windows`, `platform: macos`, `platform: linux` — behaviour
  specific to one system.

Whether it can start:

- `needs-decision` — waits on a decision about how the app should behave.
  Comments on the issue help make it.
- `blocked` — waits on something outside the project, such as a
  dependency's release.

After an issue is opened:

- `question` — work waits on more information, usually from whoever
  opened the issue. Despite its name, it does not mark an issue that asks
  a question.
- `help wanted` — the maintainer would welcome a contributor taking it on.
- `duplicate` — another issue already covers it; it is closed with a link
  to that one.
- `invalid` — not a problem in photography-workbench; it is closed with the
  reason.
- `wontfix` — decided against; it is closed, and the issue says why.

## Security issues

Please **don't** report security problems in a public issue. See
[SECURITY.md](SECURITY.md) for how to report them privately.

## Becoming a contributor

Pull requests come from contributors. While the project is young, a pull
request from someone who is not one yet is closed with a link to this section,
however good the change.

You become a contributor by taking part first:

1. **Start in [Discussions](https://github.com/kuolas/photography-workbench/discussions).**
   Say who you are and what brings you to the project.
2. **Use the app** on your own photos, on your own system.
3. **Report what you find**: bug reports and feature requests that come from
   that use, through the issue forms.

A contributor also has a public GitHub profile with a visible record of
activity, such as projects, issues and reviews, so the maintainer can see how
they work.

After you have taken part this way for a while, the maintainer invites you to
work on an issue. Agree on the approach in that issue before you start.

## Making a change

Every change reaches `master` through a pull request — including the
maintainer's own. The flow:

1. **Fork** the repository (or, if you have write access, work in it
   directly) and create a branch from an up-to-date `master`:

   ```sh
   git fetch origin
   git switch -c wip/short-description origin/master
   ```

   Branch names follow GNOME's convention: `wip/short-description` or
   `wip/yournick/short-description`, lower-case, using digits and `. _ -`.
   The `wip/` prefix says the branch's history may still be rewritten — which
   it will be, when you rebase it. If you work from a fork, branch from
   `upstream/master` instead (`git remote add upstream
   https://github.com/kuolas/photography-workbench.git` once).

2. **Commit in small, atomic steps.** One logical change per commit, and
   **every commit must build and pass the tests on its own** — not just the
   last one. This is what keeps `git bisect` useful, and it is the one rule
   that cannot be fixed after the fact. A few habits that make it easy:

   - Keep refactoring and behaviour changes in separate commits.
   - Put tests and docs in the same commit as the change that needs them.
   - Fix a mistake from an earlier commit with `git commit --fixup=<sha>` and
     fold it in with `git rebase -i --autosquash origin/master`, rather than
     adding a "fix the previous commit" commit.

3. **Run the checks** (see [Code style and checks](#code-style-and-checks)).

4. **Rebase onto the latest `master`** — never merge `master` into your
   branch:

   ```sh
   git fetch origin
   git rebase origin/master
   git push --force-with-lease   # only your own branch, never master
   ```

5. **Open a pull request.** Describe what changed and why, and link the issue
   it answers. The template has a short checklist. Give the PR a title in the
   same shape as a commit subject (`catalog: Read EXIF dates`).

### Linear history: why rebase, not merge

`master` has a **linear history**: no merge commits and no squashing. Pull
requests are landed with GitHub's *Rebase and merge*, so every commit you
wrote becomes a commit on `master`. That is why each one has to build and pass
on its own, and why we ask you to rebase rather than merge. The payoff is a
history that reads as a sequence of small, reviewable steps, reverts that touch
exactly one idea, and `git bisect` runs that stop on the commit that actually
caused a problem.

One consequence worth knowing: GitHub's rebase-merge always writes new commits
with new hashes. When you refer to a commit, use its hash **on `master`**, not
the one from your branch.

### Landing a pull request (maintainers)

Before merging, with the branch rebased onto the current `origin/master`:

```sh
scripts/check-series.sh              # every commit builds and passes tests
gh pr merge --rebase --delete-branch
```

`scripts/check-series.sh` builds and tests each commit of the branch in a
throwaway worktree, so your checkout is untouched. It refuses a branch that is
not on top of the base or that contains a merge commit. `--quick` builds
without testing; commits that touch only docs are skipped unless you pass
`--all`. For a fork, pass the base explicitly: `scripts/check-series.sh
upstream/master`. If `master` moves before the merge, rebase and run it again.

## Commit messages

We follow the
[GNOME commit message guidelines](https://handbook.gnome.org/development/commit-messages.html):

```
catalog: Read the capture date from EXIF

Sorting by file modification time put every photo copied off a card on
the day it was copied. The capture date is what a person means by "when
was this taken", and every camera writes it.

Closes: #12
```

- **Subject line:** `component: Summary`.
  - The component tag is optional and a single word naming the part of the
    project the commit touches: a crate (`catalog`, `previews`,
    `desktop-ui`, or `app` for `photography-workbench`), or `docs`, `build`,
    `ci`.
  - The summary is in the imperative ("Add", not "Added"), **starts with a
    capital letter** (or with an identifier written as it appears in code,
    ``previews: `scale_to()` keeps the aspect ratio``), has **no trailing
    period**, and stays within about **72 characters**.
- **A blank second line**, always.
- **Body:** plain prose wrapped at 75 columns, explaining *why* the change is
  needed — the diff already shows *how*. If the commit fixes a bug introduced
  by an earlier commit, name it in the body as `64aee44 ("catalog: Read dates
  lazily")`, using the hash on `master`.
- **Issue references** go in the last paragraph, one per line:
  - `Closes: #12` or `Fixes: #12` closes the issue when the commit reaches
    `master` — put it on the commit that actually completes the work;
  - `Issue: #12` refers to an issue without closing it, as each commit of a
    series toward one issue does.

  The value is `#12`, `owner/repo#12` or a full URL.
- **Commits are authored by their human author.** Disclose significant AI help
  with an `Assisted-by:` trailer (see [AI-assisted contributions](#ai-assisted-contributions));
  don't name a tool in `Co-Authored-By:` or add a "Generated with …" footer.
  `Signed-off-by:` is not needed.

More examples:

```
previews: Cache scaled images by content hash
build: Pin the Rust toolchain for CI
Add a README
```

Git's own subjects (`Revert "…"`, `fixup! …`, `squash! …`) are accepted as
they are; for a revert, add the reason to the body.

## AI-assisted contributions

You may use AI tools. We follow the
[Fedora AI-Assisted Contributions Policy](https://docs.fedoraproject.org/en-US/council/policy/ai-contribution-policy/);
the full text as it applies here is in
[ADR006](docs/architecture-decisions/adr006-ai-assisted-contributions.md). In
short:

- **You are accountable.** You are always the author, and you vouch for the
  quality, license compliance (GPL-3.0-or-later) and usefulness of
  everything you submit, however it was produced.
- **Disclose.** You must disclose AI use when a significant part of the
  contribution is taken from a tool without changes, and should disclose
  other uses where it might be useful. In commits, use an `Assisted-by:`
  trailer; in issues and comments, a short note:

  ```
  previews: Cache scaled images by content hash

  …

  Assisted-by: Claude
  ```

  Help with grammar, spelling or clarity needs no disclosure.
- **A human decides.** AI may assist reviewers, but never as the sole or
  final judge of a contribution, nor to evaluate a person's standing in the
  community.
- **Large-scale initiatives** — bots or agents opening pull requests, mass
  changes — need an issue and the maintainers' agreement first.

Concerns about a possible violation go privately to the maintainer (address
in [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)), not to a public issue.

## Code style and checks

Before you open a pull request, run these four commands — the same ones CI
runs, in the same order:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
```

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs them on every
pull request on macOS, Linux and Windows, together with the commit-message
check, a check that there are no merge commits in the PR, and
`scripts/check-crate-boundary.sh`, which checks that the crates depend on
one another only in the direction
[ADR013](docs/architecture-decisions/adr013-a-workspace-of-small-crates.md)
sets; run it yourself after adding a dependency. CI is the real gate: a
pull request is merged only when all of its checks pass.

A change to how the window looks can be reviewed in screenshots. This
test, run only when asked for, draws the window in every theme and
writes a PNG file of each scene to the folder `PW_SCREENSHOTS` names;
`PW_SCREENSHOT_PHOTOS`, if set, names a folder of photos to show in
place of a few plain JPEGs:

```sh
PW_SCREENSHOTS=/path/to/folder cargo test -p photography-workbench \
    --lib screenshots -- --ignored
```

A few conventions:

- **Bug fixes come with a regression test** — one you have seen fail without
  the fix and pass with it.
- **A user-visible change adds a line to [CHANGELOG.md](CHANGELOG.md)** under
  `[Unreleased]`, in the same pull request. Internal refactors and docs
  changes don't need one.
- **Comments describe the code as it is**, not its history ("added in v0.2"
  belongs in git). A comment marking a known shortcut cites its issue —
  `// See #123` — rather than a bare `TODO`.
- **New source files start with the license header:**
  `// SPDX-License-Identifier: GPL-3.0-or-later`.
- **Decisions that would surprise someone reading only the code** get a short
  Architecture Decision Record in
  [docs/architecture-decisions/](docs/architecture-decisions/README.md).

## Local git hooks (optional)

The repository ships git hooks that catch the common mistakes before they
reach CI. They are optional — CI checks the same things — but they save a
round trip:

```sh
scripts/install-hooks.sh          # enable them for this checkout
scripts/install-hooks.sh --check  # report whether they are enabled
```

The script points `core.hooksPath` at [`.githooks/`](.githooks/) and sets
`pull.ff only`, so a `git pull` that would create a merge commit stops with an
error instead.

- `commit-msg` checks the commit message against the active grammar.
- `pre-merge-commit` and `pre-commit` refuse merge commits and squash merges
  on any branch — rebase instead.
- `pre-push` refuses a direct push to `master` or a push containing a merge
  commit, then runs `cargo fmt --check` and `cargo clippy`.

To skip them once, set `SKIP_HOOKS=1` (for example `SKIP_HOOKS=1 git push`).

If you change a hook or a script, keep it executable in git:
`git update-index --chmod=+x <file>`. CI checks this.

## License of contributions

photography-workbench is licensed under the
[GNU General Public License v3.0 or later](LICENSE). By submitting a
contribution, you agree that it is licensed under the same terms. There is no
contributor license agreement (CLA), and you keep the copyright to your work.
