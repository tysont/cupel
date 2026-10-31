#!/bin/sh
# ABOUTME: Creates an agent worktree under the main checkout's .claude/worktrees, and removes one together with what
# ABOUTME: it alone holds outside itself, its mutation slots (ADR-156 §5). `make worktree` and `make worktree-remove` run it.
set -eu

fail() {
	echo "worktree.sh: $*" >&2
	exit 1
}

usage="usage: worktree.sh add NAME [BASE] | worktree.sh remove NAME"
if [ $# -lt 2 ] || [ -z "$2" ]; then
	fail "$usage"
fi
action=$1
name=$2

# The main checkout holds the repository every worktree shares; agent worktrees live in its
# .claude/worktrees, which its .gitignore leaves out, whichever checkout this runs from.
common=$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null) ||
	fail "not inside a git checkout"
main=$(cd "$common/.." && pwd -P)
path=$main/.claude/worktrees/$name

case $action in
add)
	base=${3:-main}
	if git show-ref --verify --quiet "refs/heads/$name"; then
		git worktree add "$path" "$name"
	else
		git worktree add -b "$name" "$path" "$base"
	fi
	;;
remove)
	[ -d "$path" ] || fail "no worktree at $path"
	path=$(cd "$path" && pwd -P)
	case $(pwd -P)/ in
	"$path"/*) fail "cannot remove $path from inside it; run this from the main checkout" ;;
	esac
	# The mutation engine keeps each repository's slots under $TMPDIR/cupel-mutate/<key>, naming the
	# repository's canonical path in the slot directory's `repository` file (ADR-141 §3); slot 0
	# holds a whole build of the workspace.
	slots=${TMPDIR:-/tmp}
	for owner in "${slots%/}"/cupel-mutate/*/repository; do
		[ -f "$owner" ] || continue
		if [ "$(cat "$owner")" = "$path" ]; then
			rm -rf "${owner%/repository}"
			echo "worktree.sh: removed the mutation slots ${owner%/repository}"
		fi
	done
	# --force: the worktree goes with its untracked and ignored files, its target/ (the coverage
	# build under it included) and .cupel/. The branch stays.
	git worktree remove --force "$path"
	echo "worktree.sh: removed $path"
	;;
*)
	fail "$usage"
	;;
esac
