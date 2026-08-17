import { createCollection } from '@tanstack/react-db';
import { queryCollectionOptions } from '@tanstack/query-db-collection';
import type { QueryClient } from '@tanstack/react-query';
import {
  reminderOperation,
  type CategoryList,
  type Reminder,
  type ReminderCategory,
  type ReminderList,
} from './reminders';

function changed<T>(before: T, after: T): boolean {
  return JSON.stringify(before) !== JSON.stringify(after);
}

function reminderCreateArgs(reminder: Reminder): Record<string, unknown> {
  return {
    id: reminder.id,
    title: reminder.title,
    text: reminder.text,
    completed: reminder.completed,
    favorite: reminder.favorite,
    categoryId: reminder.categoryId,
    reminderAt: reminder.reminderAt || reminder.startsAt,
    allDay: reminder.allDay,
    repeat: reminder.repeat,
    alertType: reminder.alertType,
    earlyAlert: reminder.earlyAlert,
    checklist: reminder.checklist,
    location: reminder.location,
  };
}

function reminderUpdateArgs(original: Reminder, modified: Reminder): Record<string, unknown> {
  const args: Record<string, unknown> = { id: modified.id };
  for (const key of ['title', 'text', 'completed', 'favorite', 'categoryId', 'alertType'] as const) {
    if (original[key] !== modified[key]) args[key] = modified[key];
  }
  if (original.allDay !== modified.allDay) args.allDay = modified.allDay;
  if (original.reminderAt !== modified.reminderAt
    || original.startsAt !== modified.startsAt
    || original.allDay !== modified.allDay) {
    args.reminderAt = modified.reminderAt || modified.startsAt;
  }
  if (changed(original.repeat, modified.repeat)) args.repeat = modified.repeat;
  if (changed(original.earlyAlert, modified.earlyAlert)) args.earlyAlert = modified.earlyAlert;
  if (changed(original.checklist, modified.checklist)) args.checklist = modified.checklist;
  if (changed(original.location, modified.location)) args.location = modified.location;
  return args;
}

function categoryUpdateArgs(original: ReminderCategory, modified: ReminderCategory): Record<string, unknown> {
  const args: Record<string, unknown> = { id: modified.id };
  for (const key of ['name', 'color', 'iconIndex', 'order'] as const) {
    if (original[key] !== modified[key]) args[key] = modified[key];
  }
  return args;
}

export function createReminderCollections(queryClient: QueryClient, endpoint: string) {
  const reminders = createCollection(queryCollectionOptions({
    queryKey: ['reminders', endpoint],
    queryFn: () => reminderOperation<ReminderList>('list', { limit: 500 }, endpoint),
    select: (data: ReminderList) => data.reminders,
    queryClient,
    getKey: (reminder: Reminder) => reminder.id,
    onInsert: async ({ transaction }) => {
      for (const mutation of transaction.mutations) {
        await reminderOperation('create', reminderCreateArgs(mutation.modified), endpoint);
      }
    },
    onUpdate: async ({ transaction }) => {
      for (const mutation of transaction.mutations) {
        await reminderOperation('update', reminderUpdateArgs(mutation.original, mutation.modified), endpoint);
      }
    },
    onDelete: async ({ transaction }) => {
      for (const mutation of transaction.mutations) {
        await reminderOperation('delete', { id: mutation.key, confirmId: mutation.key }, endpoint);
      }
    },
  }));

  const categories = createCollection(queryCollectionOptions({
    queryKey: ['reminder-categories', endpoint],
    queryFn: () => reminderOperation<CategoryList>('list_categories', {}, endpoint),
    select: (data: CategoryList) => data.categories,
    queryClient,
    getKey: (category: ReminderCategory) => category.id,
    onInsert: async ({ transaction }) => {
      for (const mutation of transaction.mutations) {
        const category = mutation.modified;
        await reminderOperation('create_category', {
          id: category.id,
          name: category.name,
          color: category.color,
          iconIndex: category.iconIndex,
          order: category.order,
        }, endpoint);
      }
    },
    onUpdate: async ({ transaction }) => {
      for (const mutation of transaction.mutations) {
        await reminderOperation('update_category', categoryUpdateArgs(mutation.original, mutation.modified), endpoint);
      }
    },
    onDelete: async ({ transaction }) => {
      for (const mutation of transaction.mutations) {
        await reminderOperation('delete_category', {
          id: mutation.key,
          confirmId: mutation.key,
        }, endpoint);
      }
      await reminders.utils.refetch();
    },
  }));

  return { reminders, categories };
}
