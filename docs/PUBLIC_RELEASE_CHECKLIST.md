# Public release checklist

This checklist records the source and release controls used for the initial public release. It is not legal advice.

## Completed source checks

- [x] MIT license and public repository metadata are present.
- [x] The Tauri identifier and Credential Manager target use project-controlled names.
- [x] A final source-only privacy scan found no credentials, private keys, personal account data, captured payloads, APKs, decompiled source, screenshots, databases, dumps, or local paths.
- [x] The MCP status operation redacts account email and account-ID hints.
- [x] CDP accepts only an explicit `http://127.0.0.1:<port>` origin.
- [x] Live Samsung collections and account probes cannot mount before explicit first-run consent.
- [x] Dependency locks, pinned CI actions, fixed Rust toolchain, RustSec audit, and Dependabot configuration are committed.
- [x] Third-party notices, provenance, security boundaries, and private-interface caveats are documented.
- [x] CI checks frontend types/build output plus Rust formatting, linting, and tests on Windows x64; release CI repeats the Rust gates on native x64 and ARM64 runners.

## Release checks

- [x] The workflow builds x64 and ARM64 NSIS, MSI, separate desktop/MCP portable ZIPs, checksums, and Scoop artifacts from the same tested version.
- [x] A read-only build job is separated from the write-capable publish job.
- [x] The publish job refuses to release if `main` changed during the build.
- [x] The desktop and MCP Scoop manifests are generated from their final portable ZIP hashes.
- [ ] Add Authenticode signing when a protected signing identity is available. Until then, releases must state that Windows may show an unknown-publisher warning.
- [ ] Smoke-test portable install, update, and uninstall on clean Windows x64 and ARM64 machines for each release.

## Owner decisions and ongoing risk

- The owner selected MIT for original project material.
- Samsung's private interfaces and signed-extension behavior are unsupported and can change or be blocked.
- The publisher must independently evaluate Samsung terms and applicable interoperability law before distributing in each jurisdiction. See [LEGAL.md](../LEGAL.md).
- The name is used to identify compatibility. Branding must remain visibly unofficial and must not imply Samsung endorsement.

Tauri's generated capability schemas under `src-tauri/gen/schemas` are intentionally retained for editor validation and change tracking; they contain no local account data.
