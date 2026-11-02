import { useSyncExternalStore } from 'react';
import en from './en.json';
import ru from './ru.json';
import { createLanguageStore, translate } from './core';
import type { LanguagePreference, MessageParams } from './core';

const browserStorage = {
  getItem: (key: string) => typeof window === 'undefined' ? null : window.localStorage.getItem(key),
  setItem: (key: string, value: string) => { if (typeof window !== 'undefined') window.localStorage.setItem(key, value); },
};
const store = createLanguageStore(browserStorage, () => typeof navigator === 'undefined' ? [] : navigator.languages?.length ? navigator.languages : [navigator.language]);
const catalogs = { en, ru };
const updateDocumentLanguage = () => { if (typeof document !== 'undefined') document.documentElement.lang = store.getLanguage(); };
updateDocumentLanguage();

export const getLocale = (): string => store.getLanguage() === 'ru' ? 'ru-RU' : 'en-US';
export const t = (key: string, params?: MessageParams): string => translate(catalogs, store.getLanguage(), key, params);
export function formatDate(value: string | number | Date, options?: Intl.DateTimeFormatOptions): string {
  const date = value instanceof Date ? value : new Date(value);
  return Number.isNaN(date.getTime()) ? '' : new Intl.DateTimeFormat(getLocale(), options).format(date);
}
export const formatNumber = (value: number, options?: Intl.NumberFormatOptions): string => new Intl.NumberFormat(getLocale(), options).format(value);

let subscribers = 0;
const refresh = () => { store.refresh(); updateDocumentLanguage(); };
const onStorage = (event: StorageEvent) => {
  if (event.key !== null && event.key !== 'reminder-language') return;
  // Ignore another storage area (for example sessionStorage) using the same key.
  try { if (event.storageArea && event.storageArea !== window.localStorage) return; } catch { return; }
  refresh();
};
const subscribe = (listener: () => void) => {
  subscribers++;
  const unsubscribe = store.subscribe(() => { updateDocumentLanguage(); listener(); });
  if (subscribers === 1 && typeof window !== 'undefined') {
    window.addEventListener('storage', onStorage);
    window.addEventListener('languagechange', refresh);
    refresh();
  }
  return () => {
    unsubscribe();
    subscribers--;
    if (subscribers === 0 && typeof window !== 'undefined') {
      window.removeEventListener('storage', onStorage);
      window.removeEventListener('languagechange', refresh);
    }
  };
};
const setLanguage = (next: LanguagePreference) => { store.setPreference(next); updateDocumentLanguage(); };
export function useI18n() {
  useSyncExternalStore(subscribe, store.getSnapshot, () => 'system:en');
  return { t, language: store.getLanguage(), preference: store.getPreference(), setLanguage, locale: getLocale() };
}
