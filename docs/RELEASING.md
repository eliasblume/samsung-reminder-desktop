# Releasing

Releases are produced only by the manually dispatched `Release` workflow on `main`.

## Version choices

- `current` publishes the committed version without incrementing it. It is intended for the initial `v0.1.0` release only.
- `patch`, `minor`, and `major` apply a stable SemVer increment to `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.
- The workflow refuses to reuse an existing tag.

## Build and publish boundary

The native Windows x64 and ARM64 matrix jobs have read-only repository access. Each applies the prepared candidate version, installs locked dependencies, runs frontend and Rust checks, builds NSIS/MSI and MCP binaries, creates separate desktop and MCP portable ZIPs, and derives architecture-specific checksums and Scoop fragments from those ZIPs. The ARM64 runner currently uses GitHub's `windows-11-arm` public-preview image.

Only after those steps succeed does a separate write-capable job verify that remote `main` is still the exact source revision. It commits the tested version files and both Scoop manifests, creates the annotated `vMAJOR.MINOR.PATCH` tag, pushes both, creates a draft GitHub release with every artifact, then publishes it.

If `main` changed during the build, rerun the workflow. If a push succeeds but release publication fails, inspect the tag and any draft release before retrying; do not delete or overwrite a public tag without reviewing downloaded artifacts and repository state.

## Outputs

- `Samsung-Reminder-VERSION-windows-ARCH-setup.exe`
- `Samsung-Reminder-VERSION-windows-ARCH.msi`
- `Samsung-Reminder-VERSION-windows-ARCH.zip`
- `Samsung-Reminder-VERSION-windows-ARCH.zip.sha256`
- `Samsung-Reminder-MCP-VERSION-windows-ARCH.zip`
- `Samsung-Reminder-MCP-VERSION-windows-ARCH.zip.sha256`
- `samsung-reminder.json`
- `samsung-reminder-mcp.json`

`ARCH` is `x64` or `arm64`. The desktop ZIP contains `Reminder.exe`; the MCP ZIP contains `samsung-reminder-mcp.exe`. Both include the README, MIT license, legal notice, and third-party notices. The merged multi-architecture Scoop manifests are committed at `bucket/samsung-reminder.json` and `bucket/samsung-reminder-mcp.json`.

The initial artifacts are unsigned. Add Authenticode signing before packaging and checksumming when a protected signing identity is available.
