import assert from 'node:assert/strict';
import { register } from 'node:module';
import { test } from 'node:test';
import { QueryClient } from '@tanstack/react-query';

// Node's stripped TypeScript loader requires extensions; production uses Vite.
register('data:text/javascript,' + encodeURIComponent(`
export async function resolve(specifier, context, nextResolve) {
  try { return await nextResolve(specifier, context); }
  catch (error) {
    if (specifier.startsWith('.') && context.parentURL?.endsWith('.ts')) return nextResolve(specifier + '.ts', context);
    throw error;
  }
}`), import.meta.url);globalThis.window = {};
globalThis.localStorage = { getItem: () => null };
const { createReminderCollections } = await import('../src/lib/reminder-collections.ts');
const pause = () => new Promise(resolve => setTimeout(resolve, 250));

test('cleanup of a retired collection cannot remove a live collection query', async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const old = createReminderCollections(client, 'synthetic', false); old.reminders.startSyncImmediate();
  await old.reminders.utils.refetch();
  const current = createReminderCollections(client, 'synthetic', true);
  current.reminders.startSyncImmediate();
  const oldSub = old.reminders.subscribeChanges(() => {});
  const currentSub = current.reminders.subscribeChanges(() => {});
  try {
    await current.reminders.utils.refetch();
    assert.equal(current.reminders.size, 4);
    assert.equal(client.getQueryCache().getAll().filter(query => query.queryKey[0] === "reminders").length, 2, "independent collection owners share a query");
    const currentQuery = client.getQueryCache().getAll().filter(query => query.queryKey[0] === "reminders").at(-1);
    let removedCurrent = false;
    const stop = client.getQueryCache().subscribe(event => { if(event.type === "removed" && event.query === currentQuery) removedCurrent = true; });
    oldSub.unsubscribe();
    await old.reminders.cleanup();
    stop();
    assert.equal(removedCurrent, false, "retired owner removed current query");
    const retained = client.getQueryCache().getAll().filter(query => query.queryKey[0] === 'reminders');
    assert.ok(retained.includes(currentQuery), 'retired owner removed the current owner cache');
    assert.equal(current.reminders.size, 4);

  } finally {
    oldSub.unsubscribe(); currentSub.unsubscribe();
    await Promise.all([old.reminders.cleanup(), old.categories.cleanup(), current.reminders.cleanup(), current.categories.cleanup()]);
    client.clear();
  }
});

test('disabled stable collections start only on explicit synchronization and retain rows on status errors', async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const collections = createReminderCollections(client, 'synthetic', false);
  const subscription = collections.reminders.subscribeChanges(() => {});
  try {
    await pause();
    assert.equal(collections.reminders.size, 0);
    await collections.reminders.utils.refetch();
    assert.equal(collections.reminders.size, 4);
    const status = client.getQueryCache().build(client, { queryKey: ['reminder-status', 'synthetic'] });
    status.setState({ status: 'error', error: new Error('synthetic offline') });
    assert.equal(collections.reminders.size, 4);
    await collections.reminders.utils.refetch();
    assert.equal(collections.reminders.size, 4);
  } finally {
    subscription.unsubscribe();
    await Promise.all([collections.reminders.cleanup(), collections.categories.cleanup()]);
    client.clear();
  }
});