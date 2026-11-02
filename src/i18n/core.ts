export type Language = 'en' | 'ru';
export type LanguagePreference = 'system' | Language;
export type Catalog = Record<string, string | Record<string, string>>;
export type MessageParams = Record<string, string | number>;
type Storage = { getItem(key: string): string | null; setItem(key: string, value: string): void };
const STORAGE_KEY = 'reminder-language';
const isPreference = (value: unknown): value is LanguagePreference => value === 'system' || value === 'en' || value === 'ru';

export function resolveLanguage(preference: LanguagePreference, systemLanguages: readonly string[]): Language {
  if (preference !== 'system') return preference;
  for (const locale of systemLanguages) {
    const language = locale.toLowerCase().split(/[-_]/)[0];
    if (language === 'en' || language === 'ru') return language;
  }
  return 'en';
}

export function translate(catalogs: Partial<Record<string, Catalog>>, language: string, key: string, params: MessageParams = {}): string {
  const locale = language === 'ru' ? 'ru' : 'en';
  let message = catalogs[locale]?.[key];
  let messageLanguage: Language = locale;
  if (message === undefined) { message = catalogs.en?.[key]; messageLanguage = 'en'; }
  if (message === undefined) return key;
  let template: string | undefined;
  if (typeof message === 'string') template = message;
  else {
    const form = typeof params.count === 'number' ? new Intl.PluralRules(messageLanguage).select(params.count) : 'other';
    template = message[form] ?? message.other;
  }
  if (typeof template !== 'string' || !template.trim()) {
    if (messageLanguage !== 'en') return translate(catalogs, 'en', key, params);
    return key;
  }
  return template.replace(/\{([a-zA-Z_][\w]*)\}/g, (placeholder, name: string) => {
    const value = params[name];
    return value === undefined ? placeholder : typeof value === 'number' ? new Intl.NumberFormat(locale).format(value) : value;
  });
}

const placeholders = (text: string) => [...new Set([...text.matchAll(/\{([a-zA-Z_][\w]*)\}/g)].map((match) => match[1]))].sort().join(',');
export function validateCatalog(base: Catalog, translated: Catalog, language: string): string[] {
  const errors: string[] = [];
  for (const key of Object.keys(translated)) if (!Object.hasOwn(base, key)) errors.push(`${key}: unexpected key`);
  for (const [key, original] of Object.entries(base)) {
    const value = translated[key];
    if (value === undefined) { errors.push(`${key}: missing translation`); continue; }
    if (typeof original === 'string') {
      if (typeof value !== 'string') { errors.push(`${key}: expected a string`); continue; }
      if (!value.trim()) errors.push(`${key}: empty translation`);
      if (placeholders(original) !== placeholders(value)) errors.push(`${key}: interpolation parameters differ`);
    } else {
      if (typeof value !== 'object' || value === null || Array.isArray(value)) { errors.push(`${key}: expected plural forms`); continue; }
      const required = language === 'ru' ? ['one', 'few', 'many', 'other'] : Object.keys(original);
      for (const form of required) if (!Object.hasOwn(value, form)) errors.push(`${key}.${form}: missing plural form`);
      for (const [form, text] of Object.entries(value)) {
        if (!['zero', 'one', 'two', 'few', 'many', 'other'].includes(form)) errors.push(`${key}.${form}: invalid plural form`);
        if (typeof text !== 'string' || !text.trim()) { errors.push(`${key}.${form}: empty translation`); continue; }
        if (placeholders(original[form] ?? original.other ?? '') !== placeholders(text)) errors.push(`${key}.${form}: interpolation parameters differ`);
      }
    }
  }
  return errors;
}

export function createLanguageStore(storage: Storage | undefined, getSystemLanguages: () => readonly string[]) {
  let preference: LanguagePreference = 'system';
  let localPreferenceOnly = false;
  const readPreference = () => {
    if (!storage || localPreferenceOnly) return preference;
    try { const saved = storage.getItem(STORAGE_KEY); return isPreference(saved) ? saved : 'system'; }
    catch { return preference; }
  };
  preference = readPreference();
  let language = resolveLanguage(preference, getSystemLanguages());
  let snapshot = `${preference}:${language}`;
  const listeners = new Set<() => void>();
  const update = () => {
    language = resolveLanguage(preference, getSystemLanguages());
    const next = `${preference}:${language}`;
    if (next === snapshot) return;
    snapshot = next;
    for (const listener of listeners) listener();
  };
  return {
    getPreference: () => preference,
    getLanguage: () => language,
    getSnapshot: () => snapshot,
    setPreference: (next: LanguagePreference) => {
      if (!isPreference(next)) return;
      preference = next;
      try { storage?.setItem(STORAGE_KEY, next); localPreferenceOnly = false; } catch { localPreferenceOnly = true; }
      update();
    },
    subscribe: (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener); }; },
    refresh: () => { preference = readPreference(); update(); },
  };
}
