# Desktop localization implementation plan

> **For agentic workers:** Use superpowers:subagent-driven-development for isolated UI/catalog work and read-only synchronization diagnosis. The coordinator owns the translation core, integration, and native verification.

**Goal:** Complete Russian/English UI with persistent language selection and verified stable reminder loading.

**Architecture:** Separate JSON catalogs feed a pure translation/validation core and a small React external store. The existing UI calls the translator without changing reminder data or backend protocol.

**Tech Stack:** React 19, TypeScript 7, Intl, Node test runner, Tauri 2.

## 1. Translation core

Files: `src/i18n/core.ts`, `src/i18n/index.ts`, `tests/i18n.test.mjs`.

- [x] Write tests for catalog presence/completeness, parameters, Russian plurals,
  English fallback, system-language resolution, and persisted preference.
- [x] Run `node --experimental-strip-types --test tests/i18n.test.mjs`; observe
  the missing catalog assertion before implementation.
- [x] Implement these interfaces:

```ts
type LanguagePreference = 'system' | 'en' | 'ru';
type MessageValue = string | Record<string, string>;
type Catalog = Record<string, MessageValue>;
resolveLanguage(preference: LanguagePreference, systemLanguages: readonly string[]): 'en' | 'ru';
translate(catalogs: Record<string, Catalog>, language: string, key: string, params?: Record<string, string | number>): string;
validateCatalog(base: Catalog, translated: Catalog, language: string): string[];
```

- [x] Provide `createLanguageStore(storage, getSystemLanguages)` with
  `getPreference`, `getLanguage`, `getSnapshot`, `setPreference`, `subscribe`, and
  `refresh`. Catch unavailable storage while preserving the in-memory preference.
- [x] Provide `t`, `formatDate`, `formatNumber`, `getLocale`, and `useI18n`.
  The hook returns `language`, `preference`, `setLanguage`, `locale`, and `t`.
- [x] Rerun tests and commit the independently working core/catalog integration.

## 2. Complete UI catalogs

Files: `src/App.tsx`, `src/lib/category-icons.ts`, `src/i18n/en.json`, `src/i18n/ru.json`.

- [x] Extract every authored visible string and accessible label into semantic keys.
- [x] Translate all Russian entries, including disclosure and settings dialogs.
- [x] Add Settings selection with these values:

```ts
[
  { value: 'system', label: t('language.system') },
  { value: 'ru', label: 'Русский' },
  { value: 'en', label: 'English' },
]
```

- [x] Subscribe top-level UI to `useI18n`; include language in memoized translated
  category/group labels. Replace fixed locale formatting with `getLocale()` or
  `formatDate`. Use plural messages for counts.
- [x] Preserve stored user content and machine-readable configuration examples.
- [x] Run catalog tests, `pnpm typecheck`, and `pnpm build`.

## 3. Synchronization lifecycle

Files: `src/lib/reminder-collections.ts`, `src/App.tsx`, regression tests.

- [x] Reproduce the reported rows-to-zero transition in a synthetic isolated page,
  recording collection identity, subscribers, rows, cache events, and query result.
- [x] Add a failing regression for the observed transition and fix its owning layer.
- [ ] Verify transient failure retains successfully loaded rows and reports failure.
- [x] Verify cleanup cannot delete another live collection's query.

## 4. Delivery

- [ ] Review all changes and document adding a language in README.
- [ ] Run frontend tests/types/build and Rust checks affected by the final diff.
- [ ] Commit, verify `gh auth status` is active `korsun009`, push, and update PR 53.
- [ ] Build the final native installer using the authorized temporary SSD target.
- [ ] Install and verify installer-payload hash, manual sync, restart, both languages,
  saved preference, and stable counters past the cleanup interval.
- [ ] Copy final artifacts/provenance to outputs, then safely remove the authorized
  temporary SSD directory after resolving and checking its exact absolute path.
