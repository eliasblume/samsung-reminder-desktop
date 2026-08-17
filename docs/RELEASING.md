# Releasing

Releases are produced only by the manually dispatched `Release` workflow on `main`.

## Version choices

- `current` publishes the committed version without incrementing it. It is intended for the initial `v0.1.0` release only.
- `patch`, `minor`, and `major` apply a stable SemVer increment to `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.
- The workflow refuses to reuse an existing tag.

## Build and publish boundary

The Windows x64 build job has read-only repository access. It bumps the candidate version in its workspace, installs locked dependencies, runs frontend and Rust checks, builds NSIS/MSI and MCP binaries, creates a portable ZIP, and derives both the checksum and Scoop manifest from that ZIP.

Only after those steps succeed does a separate write-capable job verify that remote `main` is still the exact source revision. It commits the tested version files and Scoop manifest, creates the annotated `vMAJOR.MINOR.PATCH` tag, pushes both, creates a draft GitHub release with every artifact, then publishes it.

If `main` changed during the build, rerun the workflow. If a push succeeds but release publication fails, inspect the tag and any draft release before retrying; do not delete or overwrite a public tag without reviewing downloaded artifacts and repository state.

## Outputs

- `Samsung-Reminder-VERSION-windows-x64-setup.exe`
- `Samsung-Reminder-VERSION-windows-x64.msi`
- `Samsung-Reminder-VERSION-windows-x64.zip`
- `Samsung-Reminder-VERSION-windows-x64.zip.sha256`
- `samsung-reminder.json`

The portable ZIP contains `Reminder.exe`, `samsung-reminder-mcp.exe`, README, MIT license, legal notice, and third-party notices. The generated Scoop manifest is committed at `bucket/samsung-reminder.json`.

The initial artifacts are unsigned. Add Authenticode signing before packaging and checksumming when a protected signing identity is available.
