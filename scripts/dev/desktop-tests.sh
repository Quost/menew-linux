#!/usr/bin/env bash
# Run the headless GNOME and KDE tests inside the dev container (scripts/dev/Dockerfile).
# bash scripts/dev/run.sh bash scripts/dev/desktop-tests.sh [gnome|kde]
set -uo pipefail
cd /src
cargo build 2>&1 | tail -1
# no logind in a container: without /run/systemd/seats GNOME Shell uses its dummy login manager
rm -rf /run/systemd/seats
# kwin_wayland carries cap_sys_nice, which a container cannot grant: drop it
command -v setcap >/dev/null && setcap -r /usr/bin/kwin_wayland 2>/dev/null
mkdir -p /run/dbus && [ -S /run/dbus/system_bus_socket ] || dbus-daemon --system --fork
export MENEW_BIN=/target/debug/menew
rc=0
for de in ${1:-gnome kde}; do
    echo "===== $de"
    timeout 600 bash "scripts/desktop-test/$de.sh" || rc=1
done
exit $rc
