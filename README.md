# Reminder for Windows

An unofficial Samsung One UI-inspired desktop client and local MCP server for reading and writing Samsung Reminder data without Microsoft Reminders sync.

> [!IMPORTANT]
> This experimental project is not affiliated with, endorsed by, or supported by Samsung. It depends on private Samsung Browser and Samsung Cloud behavior that can change without notice.

## Requirements

For a prebuilt release:

- 64-bit Windows 10 version 1809 or newer, or Windows 11. Development and release testing currently target Windows 11 x64.
- [Samsung Browser for Windows](https://browser.samsung.com/) installed using Samsung's official 64-bit installer.
- A Samsung Account signed in through Samsung Browser. Open the browser at least once so it creates the default profile.
- Network access to Samsung Cloud. The installer may also download Microsoft Edge WebView2 when it is unavailable.

This project currently expects Samsung's standard system-wide installation and default profile paths:

- `C:\Program Files\Samsung\Internet\Application\samsunginternet.exe`
- `%LOCALAPPDATA%\Samsung\Internet\User Data\Default`

Custom installation and profile locations are not currently detected. Samsung Browser must be completely closed before the first sync. Chromium otherwise routes the helper launch into the existing process without enabling the private localhost bridge. After the app has cached a valid credential, normal Reminder operations no longer keep the browser open.

Samsung currently offers separate official [x64](https://browser.samsung.com/media/x64_SamsungBrowserSetup.exe) and [ARM64](https://browser.samsung.com/media/arm64_SamsungBrowserSetup.exe) evergreen installers. This project publishes x64 builds only for now.

## Install

Download the current NSIS installer, MSI, or portable ZIP from [GitHub Releases](https://github.com/eliasblume/samsung-reminder-desktop/releases/latest).

### Scoop

This repository is also a Scoop bucket:

```powershell
scoop bucket add samsung-reminder https://github.com/eliasblume/samsung-reminder-desktop
scoop install samsung-reminder/samsung-reminder
```

The release pipeline updates the Scoop version and SHA-256 from every generated portable bundle. Future releases can then be installed with:

```powershell
scoop update samsung-reminder
```

## Run from source

Source builds require Node.js 22.12 or newer, pnpm 11.17, Rust 1.97.1, the [Tauri prerequisites for Windows](https://v2.tauri.app/start/prerequisites/), and Visual Studio Build Tools with the Desktop development with C++ workload.

```powershell
pnpm install --frozen-lockfile
pnpm tauri:dev
```

Dark mode is the first-run default. The appearance toggle is saved locally.

The editor supports Samsung lists, notes and checklists, importance, completion, alert strength, early alerts, all-day and timed schedules, place triggers, and minute/hour/day/week/month/year recurrence. List colors and all 42 category icon indices sync through Samsung Cloud. Monthly rules use a day of month and yearly rules use month plus day, matching the mobile data model.

## Build

```powershell
pnpm typecheck
pnpm tauri:build
pnpm build:mcp
```

Tauri produces NSIS and MSI installers under `src-tauri/target/release/bundle`. See [RELEASING.md](docs/RELEASING.md) for the automated SemVer release process.

## MCP server

Scoop adds `samsung-reminder-mcp` to `PATH`. A local MCP configuration can therefore use:

```toml
[mcp_servers.samsung-reminders]
command = "samsung-reminder-mcp"
env = { CDP_ENDPOINT = "http://127.0.0.1:9226" }
```

For a source build, point `command` to `src-tauri/target/release/samsung-reminder-mcp.exe` instead.

The stdio server exposes:

- `samsung_reminders_status`
- `samsung_reminders_list`
- `samsung_reminder_categories_list`
- `samsung_reminder_category_create`
- `samsung_reminder_category_update`
- `samsung_reminder_category_delete`
- `samsung_reminders_get`
- `samsung_reminders_create`
- `samsung_reminders_update`
- `samsung_reminders_delete`

Reminder and category deletion require `confirmId` to exactly match the target ID. Deleting a category first moves its reminders to My reminders. MCP status deliberately omits the Samsung account email and account-ID hint that the local desktop UI can display.

## Architecture

- React, TanStack DB, and TanStack Query for live collections, optimistic mutations, and cloud-operation caching
- Tailwind CSS, Radix primitives, and Lucide icons for the One UI-inspired interface
- Tauri 2 and Rust for the desktop shell, browser lifecycle management, and direct Samsung Cloud client
- A small JavaScript bootstrap evaluated only inside Samsung's signed Calendar extension
- Windows Credential Manager plus an in-memory Rust credential cache
- A sibling Rust stdio MCP binary sharing the same bridge implementation

## Privacy and security

The Samsung Cloud credential remains in Rust and is encrypted for the current Windows user by Windows Credential Manager. It is never returned through React or MCP and is not written to logs or plaintext files. The CDP endpoint is restricted to an explicit `http://127.0.0.1:<port>` origin.

Use **Settings → Disconnect Samsung account** to remove the cached credential and return to the first-run disclosure.

The repository contains no Samsung APKs, decompiled Samsung source, Samsung account data, captured API payloads, user screenshots, access tokens, or Samsung artwork. The extension ID, Cloud app ID, endpoint, and table names in source are protocol constants rather than user credentials.

Before the first cloud request, the app displays an explicit disclosure and requires local opt-in. See [LEGAL.md](LEGAL.md), [PROVENANCE.md](PROVENANCE.md), and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for the private-interface caveat, implementation provenance, and dependency notices.

See [SECURITY.md](SECURITY.md) before reporting a vulnerability. Never attach tokens, Reminder contents, browser-profile data, or unredacted crash dumps to a public issue.

## Scope and caveats

A Samsung Browser, signed extension, or Cloud API update can break this project. The current proof of concept covers listing, reading, creating, editing, completing, favoriting, custom lists, checklists, recurrence, place/time alarms, early alerts, all-day reminders, alert strength, sharing, and guarded deletion.

Attachment upload, voice capture, map search/geocoding, and Samsung's suggested-reminder templates are not implemented. Windows installers are initially unsigned, so Windows may display a publisher warning. Checksums verify downloaded bytes but do not establish publisher identity.

## License

[MIT](LICENSE) © 2026 eliasblume
