#!/usr/bin/env bash
# install-hooks.sh — enable the optional git hooks in .githooks/ for this
# checkout (core.hooksPath), and set pull.ff=only.
#
#   scripts/install-hooks.sh            # install (idempotent)
#   scripts/install-hooks.sh --check    # report only; non-zero if something is off
set -euo pipefail

cd "$(dirname "$0")/.."

want=".githooks"
check_only=false
[ "${1:-}" = "--check" ] && check_only=true

have="$(git config --get core.hooksPath || true)"
ff="$(git config --get pull.ff || true)"

status=0

if [ "$have" = "$want" ]; then
	echo "OK   core.hooksPath = $want"
else
	if $check_only; then
		echo "FAIL core.hooksPath is ${have:-unset} — the guards are NOT active." >&2
		echo "     Fix with: scripts/install-hooks.sh" >&2
		status=1
	else
		git config core.hooksPath "$want"
		echo "OK   core.hooksPath set to $want (was ${have:-unset})"
	fi
fi

# History is linear: a `git pull` that would create a merge commit errors
# instead.
if [ "$ff" = "only" ]; then
	echo "OK   pull.ff = only"
else
	if $check_only; then
		echo "FAIL pull.ff is ${ff:-unset} — a divergent pull would try to merge." >&2
		status=1
	else
		git config pull.ff only
		echo "OK   pull.ff set to only (was ${ff:-unset})"
	fi
fi

# Git silently skips a hook that is not executable, so check the bit too.
for hook in "$want"/*; do
	[ -f "$hook" ] || continue
	if [ -x "$hook" ]; then
		echo "OK   $(basename "$hook") is executable"
	else
		echo "FAIL $(basename "$hook") is NOT executable — git will skip it silently." >&2
		if $check_only; then
			status=1
		else
			chmod +x "$hook"
			echo "     -> fixed with chmod +x (stage it too: git update-index --chmod=+x $hook)"
		fi
	fi
done

if ! $check_only; then
	cat <<-'EOF'

		Active hooks:
		  commit-msg        the commit-message grammar (CONTRIBUTING.md)
		  pre-merge-commit  refuses any merge commit — history is linear
		  pre-commit        refuses to complete a refused merge, or a --squash merge
		  pre-push          refuses a direct push to master and pushed merge
		                    commits; runs cargo fmt --check and cargo clippy
		  pull.ff = only    a divergent pull errors instead of merging

		Skip them once with SKIP_HOOKS=1.
	EOF
fi

exit "$status"
