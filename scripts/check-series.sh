#!/usr/bin/env bash
# check-series.sh — build and test every commit on this branch, so that every
# commit landing on master is known to be green (CONTRIBUTING.md §Making a
# change). Runs in a throwaway worktree; your checkout is untouched.
#
#   scripts/check-series.sh              # origin/master..HEAD (fetched first)
#   scripts/check-series.sh <base>       # <base>..HEAD, e.g. upstream/master
#   scripts/check-series.sh --quick      # build only, skip tests
#   scripts/check-series.sh --all        # build docs-only commits too
#
# The branch must already be rebased onto the base, and hold no merge commits:
# only then are the trees built here the trees GitHub's rebase-merge lands.
# Commits that change no build input (see no_build_inputs) are not built; they
# take the previous commit's verdict.
set -euo pipefail

cd "$(dirname "$0")/.."

DEFAULT_BRANCH=master

quick=0
all=0
base=""
for arg in "$@"; do
	case "$arg" in
		--quick) quick=1 ;;
		--all) all=1 ;;
		-*) printf 'unknown flag: %s\n' "$arg" >&2; exit 2 ;;
		*) base=$arg ;;
	esac
done

if [ -z "$base" ]; then
	base="origin/$DEFAULT_BRANCH"
	# The base a PR is rebase-merged onto is the forge's master, not a local
	# copy of it: a stale one would pass a branch that is no longer on top.
	if ! git fetch --quiet origin "$DEFAULT_BRANCH" 2>/dev/null; then
		printf 'note: could not fetch %s — checking against the local copy, which may be stale\n\n' "$base" >&2
	fi
fi

if ! git rev-parse --verify -q "$base^{commit}" >/dev/null; then
	printf 'check-series: cannot resolve base %s\n' "$base" >&2
	exit 2
fi

if ! git merge-base --is-ancestor "$base" HEAD; then
	cat >&2 <<-EOT
		check-series: HEAD is not on top of $base.

		  A rebase-merge would replay these commits onto a tree they were never
		  built on, so a walk from the old base would prove nothing about what
		  lands. Rebase first, then run this again:

		    git rebase $base
	EOT
	exit 1
fi

merges=$(git rev-list --merges "$base..HEAD")
if [ -n "$merges" ]; then
	printf 'check-series: %s..HEAD holds merge commit(s) — history here is linear:\n' "$base" >&2
	git --no-pager log --no-walk --format='  %h %s' $merges >&2
	printf '\n  Replay the branch without them:  git rebase %s\n' "$base" >&2
	exit 1
fi

commits=()
while IFS= read -r line; do commits+=("$line"); done < <(git rev-list --reverse "$base..HEAD")
n=${#commits[@]}

if [ "$n" -eq 0 ]; then
	printf 'No commits between %s and HEAD.\n' "$base"
	exit 0
fi

if [ "$n" -eq 1 ]; then
	printf 'One commit — the checks on the tip already cover it.\n'
	exit 0
fi

wt=$(mktemp -d "${TMPDIR:-/tmp}/check-series.XXXXXX")
cleanup() {
	git worktree remove --force "$wt" >/dev/null 2>&1 || true
	rm -rf "$wt"
}
trap cleanup EXIT

git worktree add --detach --quiet "$wt" "$base"

# True when every path commit $1 changes is one no build, test or doctest
# reads. If a test ever reads a file under docs/ or a README, remove that path
# from the list. --no-renames: otherwise a source file moved into docs/ would
# show only its destination and pass for docs-only.
no_build_inputs() {
	local paths path
	# A commit with no parent (an orphan merged in) has no "$1^": the diff
	# fails, and a failed diff must mean "build it", never "nothing changed".
	paths=$(git diff --no-renames --name-only "$1^" "$1" 2>/dev/null) || return 1
	while IFS= read -r path; do
		case "$path" in
			'' | *.md | docs/* | .claude/* | .github/*) ;;
			LICENSE*) case "$path" in */*) return 1 ;; esac ;;
			*) return 1 ;;
		esac
	done <<<"$paths"
	return 0
}

# The build and test commands from CONTRIBUTING.md §Code style and checks, run
# in the worktree at $wt.
build_one() {
	cargo build --quiet --manifest-path "$wt/Cargo.toml" --workspace
}

test_one() {
	cargo test --quiet --manifest-path "$wt/Cargo.toml" --workspace
}

if [ "$quick" = 1 ]; then
	mode='build only'
else
	mode='build + test'
fi
printf 'Checking %d commits for per-commit greenness (%s)\n\n' "$n" "$mode"

failed=()
built=0
skipped=0
prev_ok=1
i=0
for c in "${commits[@]}"; do
	i=$((i + 1))
	subject=$(git log -1 --format='%h %s' "$c")
	printf '[%d/%d] %s\n' "$i" "$n" "$subject"

	if [ "$all" = 0 ] && no_build_inputs "$c"; then
		printf '      skipped — no build inputs, same tree as the previous commit for the toolchain\n'
		skipped=$((skipped + 1))
		ok=$prev_ok
	else
		built=$((built + 1))
		# --force: a build that rewrites a tracked file (a lockfile, say) would
		# otherwise stop the walk at the next checkout.
		git -C "$wt" checkout --quiet --force --detach "$c"

		ok=1
		build_one || ok=0
		if [ "$ok" = 1 ] && [ "$quick" = 0 ]; then
			test_one || ok=0
		fi
	fi
	prev_ok=$ok

	if [ "$ok" = 1 ]; then
		printf '      ok\n\n'
	else
		printf '      FAILED\n\n'
		failed+=("$subject")
	fi
done

if [ ${#failed[@]} -eq 0 ]; then
	printf 'All %d commits green (%d built, %d docs-only) on top of %s — safe to\n' \
		"$n" "$built" "$skipped" "$base"
	printf 'rebase-merge while %s does not move (gh pr merge --rebase --delete-branch).\n' "$base"
	exit 0
fi

printf '%d of %d commits are not green:\n' "${#failed[@]}" "$n"
printf '  %s\n' "${failed[@]}"
cat <<-EOT

	Fold the repairs into the commits they belong to before merging — every
	commit becomes a mainline commit, so there is no merge type that hides a
	red one:

	  git commit --fixup=<sha> …
	  git rebase -i --autosquash $base

	then run this again (the rebased commits are new objects nothing has built).
EOT
exit 1
