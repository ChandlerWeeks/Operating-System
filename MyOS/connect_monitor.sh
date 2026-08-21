#!/bin/bash
MONITOR=${1:-"monitor.sock"}
if [ ! -r ${MONITOR} ]; then
    echo "No monitor socket found at: ${MONITOR}."
    exit 1
fi
exec socat UNIX-CONNECT:${MONITOR} STDIO
