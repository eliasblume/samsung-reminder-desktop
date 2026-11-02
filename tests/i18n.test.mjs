import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';

const catalogPath = (language) => new URL(`../src/i18n/${language}.json`, import.meta.url);
const loadCatalog = (language) => JSON.parse(readFileSync(catalogPath(language), 'utf8'));
const core = () => import('../src/i18n/core.ts');

test('English and complete Russian catalogs are provided separately', async () => {
  assert.ok(existsSync(catalogPath('en')), 'English JSON catalog is missing');
  assert.ok(existsSync(catalogPath('ru')), 'Russian JSON catalog is missing');
  const { validateCatalog } = await core();
  const en = loadCatalog('en');
  const ru = loadCatalog('ru');
  assert.ok(Object.keys(en).length > 100, 'The catalog must cover the complete application');
  assert.deepEqual(validateCatalog(en, ru, 'ru'), []);
});

test('explicit choice overrides the system and unsupported system languages fall back to English', async () => {
  const { resolveLanguage } = await core();
  assert.equal(resolveLanguage('ru', ['en-US']), 'ru');
  assert.equal(resolveLanguage('en', ['ru-RU']), 'en');
  assert.equal(resolveLanguage('system', ['fi-FI', 'ru-RU']), 'ru');
  assert.equal(resolveLanguage('system', ['fi-FI']), 'en');
  assert.equal(resolveLanguage('system', ['RU-ru']), 'ru');
});

test('Russian count forms and interpolation follow the locale', async () => {
  const { translate } = await core();
  const catalogs = {
    en: { count: { one: '{count} reminder', other: '{count} reminders' }, saved: 'Saved {name}' },
    ru: { count: { one: '{count} напоминание', few: '{count} напоминания', many: '{count} напоминаний', other: '{count} напоминания' }, saved: 'Сохранено: {name}' },
  };
  for (const [count, expected] of [[1,'1 напоминание'],[2,'2 напоминания'],[5,'5 напоминаний'],[11,'11 напоминаний'],[21,'21 напоминание'],[1.5,'1,5 напоминания']]) {
    assert.equal(translate(catalogs, 'ru', 'count', { count }), expected);
  }
  assert.equal(translate(catalogs, 'ru', 'saved', { name: 'Список' }), 'Сохранено: Список');
  assert.equal(translate(catalogs, 'en', 'count', { count: 1 }), '1 reminder');
});

test('missing translations use English while unknown keys remain diagnosable', async () => {
  const { translate } = await core();
  const catalogs = { en: { save: 'Save', count: { one: '{count} item', other: '{count} items' } }, ru: {} };
  assert.equal(translate(catalogs, 'ru', 'save'), 'Save');
  assert.equal(translate(catalogs, 'ru', 'count', { count: 5 }), '5 items');
  assert.equal(translate(catalogs, 'xx', 'save'), 'Save');
  assert.equal(translate(catalogs, 'ru', 'missing.key'), 'missing.key');
});

test('catalog validation detects missing values, parameters, and Russian plural forms', async () => {
  const { validateCatalog } = await core();
  const errors = validateCatalog(
    { saved: 'Saved {name}', empty: 'Text', absent: 'Required', count: { one: '{count} item', other: '{count} items' } },
    { saved: 'Сохранено', empty: ' ', count: { one: '{count} элемент', other: '{count} элементов' } },
    'ru',
  );
  for (const key of ['saved','empty','absent','count']) assert.ok(errors.some((error) => error.includes(key)), `Missing validation error for ${key}`);
});

test('language preference persists and subscribers observe the resolved language', async () => {
  const { createLanguageStore } = await core();
  const entries = new Map();
  const storage = { getItem: (key) => entries.get(key) ?? null, setItem: (key,value) => entries.set(key,value) };
  const store = createLanguageStore(storage, () => ['ru-RU']);
  assert.equal(store.getPreference(), 'system');
  assert.equal(store.getLanguage(), 'ru');
  let notifications = 0;
  const unsubscribe = store.subscribe(() => notifications++);
  store.setPreference('en');
  assert.equal(store.getLanguage(), 'en');
  assert.equal(notifications, 1);
  const restarted = createLanguageStore(storage, () => ['ru-RU']);
  assert.equal(restarted.getPreference(), 'en');
  assert.equal(restarted.getLanguage(), 'en');
  unsubscribe();
});

test('unavailable storage does not prevent changing the in-memory language', async () => {
  const { createLanguageStore } = await core();
  const storage = { getItem: () => { throw Error('unavailable'); }, setItem: () => { throw Error('unavailable'); } };
  const store = createLanguageStore(storage, () => ['fi-FI']);
  assert.equal(store.getLanguage(), 'en');
  store.setPreference('ru');
  assert.equal(store.getLanguage(), 'ru');
});

test('refresh observes system and cross-window changes, including preference-only changes', async () => {
  const { createLanguageStore } = await core();
  let system = ['ru-RU'];
  let saved = null;
  const store = createLanguageStore({ getItem: () => saved, setItem: (_key, value) => { saved = value; } }, () => system);
  const original = store.getSnapshot();
  let notifications = 0;
  store.subscribe(() => notifications++);
  store.setPreference('ru');
  assert.notEqual(store.getSnapshot(), original, 'Same resolved language still needs a preference update');
  assert.equal(notifications, 1);
  store.setPreference('ru');
  assert.equal(notifications, 1, 'Stable snapshots should not notify');
  saved = 'system';
  system = ['de-DE', 'en-GB'];
  store.refresh();
  assert.equal(store.getPreference(), 'system');
  assert.equal(store.getLanguage(), 'en');
  system = ['ru'];
  store.refresh();
  assert.equal(store.getLanguage(), 'ru');
  saved = 'invalid-language';
  store.refresh();
  assert.equal(store.getPreference(), 'system');
});

test('failed persistence and absent storage keep the in-memory preference during refresh', async () => {
  const { createLanguageStore } = await core();
  for (const storage of [undefined, { getItem: () => 'en', setItem: () => { throw Error('read-only'); } }]) {
    const store = createLanguageStore(storage, () => ['en-US']);
    store.setPreference('ru');
    store.refresh();
    assert.equal(store.getPreference(), 'ru');
    assert.equal(store.getLanguage(), 'ru');
  }
});

test('English fallback uses English plural rules while interpolating the active locale', async () => {
  const { translate } = await core();
  const catalogs = { en: { count: { one: '{count} item', other: '{count} items' } }, ru: {} };
  assert.equal(translate(catalogs, 'ru', 'count', { count: 21 }), '21 items');
  assert.equal(translate(catalogs, 'ru', 'count', { count: 1.5 }), '1,5 items');
});

test('validation rejects extra keys and mismatched parameters in Russian plural forms', async () => {
  const { validateCatalog } = await core();
  const errors = validateCatalog(
    { count: { one: '{count} item for {name}', other: '{count} items for {name}' } },
    { extra: 'Лишнее', count: { one: '{count} для {name}', few: '{count}', many: '{count} для {name}', other: '{count} для {name}' } },
    'ru',
  );
  assert.ok(errors.some((error) => error.includes('extra')));
  assert.ok(errors.some((error) => error.includes('count.few')));
});
