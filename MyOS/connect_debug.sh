#!/bin/bash
DEBUG=${1:-"debug.sock"}
BIN=${2:-"target/riscv64gc-unknown-none-elf/debug/cosc562os"}
if [ ! -r $DEBUG ]; then
    echo "No debugging socket found at: ${DEBUG}."
    exit 1
elif [ ! -x $BIN ]; then
    echo "No debugging executable found at: ${BIN}."
    exit 2
fi
exec lldb -o "process connect unix-connect://${DEBUG}" ${BIN}
