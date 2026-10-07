#!/usr/bin/env bash
# Drop-in for `ssh` that runs the command inside a Proxmox LXC via the PVE host (`pct exec`),
# so the container needs no SSH server or keys of its own.
#
#   export PVE_HOST=root@<pve-host>   # required: an SSH destination for the Proxmox host
#   export PVE_CTID=200               # optional: container id (default 200)
#   export RESEARCH_SSH=research/scripts/pct-ssh.sh
#
# gate.sh calls `$RESEARCH_SSH <host> <command...>`; the <host> argument is ignored here.
set -euo pipefail
: "${PVE_HOST:?set PVE_HOST to an SSH destination for the Proxmox host, e.g. root@pve}"
shift
cmd="$*"
exec ssh -o BatchMode=yes "$PVE_HOST" "pct exec ${PVE_CTID:-200} -- bash -c $(printf '%q' "$cmd")"
