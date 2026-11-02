# Desktop localization

The user approved English/Russian localization, then explicitly chose one JSON
file per language and required complete Russian coverage.

- Store authored UI messages in `src/i18n/en.json` and `src/i18n/ru.json`.
- Use semantic message keys, named interpolation, and locale-aware plural forms.
- Provide system, English, and Russian preferences in Settings; persist locally.
- Resolve an unsupported system language to English without changing OS settings.
- Keep English fallback for unexpected missing translations; enforce complete
  Russian keys, nonempty values, interpolation parameters, and plural forms in tests.
- Translate dialogs, connection disclosures, accessible names, category icon/color
  labels, calendar text, and success/error summaries as well as the main screen.
- Format dates and counts using the selected locale. Preserve user-authored content,
  remote category names, URLs, data schemas, and MCP protocol keys.
- Keep language changes independent from collection identity and cloud loading.
- Diagnose the disappearing rows using an isolated synthetic React reproduction;
  retain the last successful data and expose connectivity failures explicitly.

Verify Node unit tests, TypeScript, the production frontend build, appropriate Rust
checks, and native behavior after installation, manual synchronization, restart,
and a period longer than the previously observed cleanup interval.
