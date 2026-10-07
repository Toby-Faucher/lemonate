#!/usr/bin/env bash
# catalog.sh: regenerate research/CATALOG.md.
exec python3 "$(dirname "${BASH_SOURCE[0]}")/catalog.py" "$@"
