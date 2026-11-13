---
title: Plugin system
column: Todo
order: 14
---
> [!NOTE]
> Plugins would be separate binaries on PATH named `mycli-<name>`, like git and cargo do it. The host passes the remaining arguments through untouched and forwards the exit code.
>
> Open questions: how plugins declare their completions, whether they can read the config file, and how `mycli --help` lists them without running each one. Look at how cargo-edit and gh extensions handle discovery before deciding.
