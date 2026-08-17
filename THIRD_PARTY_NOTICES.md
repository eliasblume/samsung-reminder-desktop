# Third-party notices

Reminder for Windows is licensed under MIT. It incorporates third-party components under their own licenses. The exact resolved versions are recorded in `pnpm-lock.yaml` and `src-tauri/Cargo.lock`; source is available from the package registries and repositories linked below.

## JavaScript and frontend packages

Resolved packages use permissive licenses including MIT, Apache-2.0, ISC, BSD-3-Clause, 0BSD, and CC0-1.0. This includes React, TanStack, Tauri JavaScript APIs, Radix UI, Tailwind CSS, TypeScript, Vite, and their transitive dependencies. Lucide icons are licensed under ISC.

- Package metadata and source links: <https://www.npmjs.com/>
- Lucide license: <https://github.com/lucide-icons/lucide/blob/main/LICENSE>

## Rust packages

Most resolved Rust crates use MIT and/or Apache-2.0 or another permissive license. The following linked transitive crates are licensed under MPL-2.0:

- `cssparser` 0.36.0 — <https://crates.io/crates/cssparser/0.36.0>
- `cssparser-macros` 0.6.1 — <https://crates.io/crates/cssparser-macros/0.6.1>
- `dtoa-short` 0.3.5 — <https://crates.io/crates/dtoa-short/0.3.5>
- `option-ext` 0.2.0 — <https://crates.io/crates/option-ext/0.2.0>
- `selectors` 0.36.1 — <https://crates.io/crates/selectors/0.36.1>

The corresponding source can be obtained from those crate pages. The MPL-2.0 text is available at <https://www.mozilla.org/MPL/2.0/>. These crates remain separately licensed; no project modification to their source is distributed.

## Desktop runtime and installer components

- Tauri and `nsis-tauri-utils`: Apache-2.0 and/or MIT, <https://github.com/tauri-apps/tauri> and <https://github.com/tauri-apps/nsis-tauri-utils>
- NSIS installer components: zlib/libpng license, with separately documented component terms, <https://nsis.sourceforge.io/Docs/AppendixI.html>
- Microsoft Edge WebView2 Runtime: installed or downloaded separately under Microsoft's terms; it is not relicensed by this project.

This notice is a practical summary, not a replacement for the complete license texts and metadata distributed by each upstream project.
