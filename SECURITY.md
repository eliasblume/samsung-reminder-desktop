# Security policy

## Reporting a vulnerability

Please use [GitHub private vulnerability reporting](https://github.com/eliasblume/samsung-reminder-desktop/security/advisories/new). Do not open a public issue containing credentials, account identifiers, reminder content, captured request headers, or a Windows crash dump.

Include a concise description, affected version, reproduction steps, and impact. Redact Samsung access tokens, user IDs, device IDs, email addresses, reminder payloads, and local filesystem paths.

## Sensitive-data boundaries

- The Samsung Cloud credential must remain in the Rust backend and Windows Credential Manager. It must not be returned through Tauri IPC or MCP.
- The CDP client accepts only an explicit `http://127.0.0.1:<port>` endpoint. Do not weaken that boundary.
- The desktop UI may display the signed-in account email locally. The MCP status tool intentionally redacts both the email and account-ID hint.
- Logs and error messages must not contain request headers, credentials, raw browser-profile data, or reminder payloads unless the user explicitly requested the reminder data through an MCP operation.
- Test fixtures and screenshots must use fictional accounts and reminder content.

## Supported versions

| Version | Supported |
| --- | --- |
| Latest GitHub release | Yes |
| Earlier releases and source snapshots | No |

The project has no automatic in-app updater. Install the newest GitHub release or run `scoop update samsung-reminder` before reporting a security issue.
