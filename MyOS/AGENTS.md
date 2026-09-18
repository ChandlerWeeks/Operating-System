# Devcontainer and QEMU testing

- From `MyOS/`, start the course container with `../.devcontainer/run_container.sh`.
- The directory is named `.devcontainer`, not `.dev_container`.
- The script opens an interactive shell and mounts the repository at `/home/cosc562/myos`.
- In the container, run `cd /home/cosc562/myos/MyOS` before you run Rust commands.
- Run `cargo check` for a source check.
- Run `cargo run` only when the user asks for a QEMU boot test. Cargo runs `run.sh`, which starts the configured RISC-V QEMU machine.
- A successful boot prints the kernel output, reaches the ELF section report, and returns to the container shell after `sbi::shutdown()`.
- Type `exit` to close the container. The launcher uses `--rm`, so Docker removes the temporary container.
- Do not run `cargo clean` unless the user asks. Preserve generated build files and unrelated working-tree changes.
