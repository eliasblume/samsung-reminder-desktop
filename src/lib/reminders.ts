import { invoke } from '@tauri-apps/api/core';

export type Reminder = {
  id: string;
  title: string;
  text: string;
  completed: boolean;
  itemStatus: number;
  eventType: number;
  eventStatus: number;
  favorite: boolean;
  categoryId: string | null;
  allDay: boolean;
  reminderAt: string | null;
  repeatType: number;
  repeatWeekdays: number;
  rrule: string | null;
  repeat: RepeatConfig | null;
  tpoType: number;
  soundType: number;
  alertType: 0 | 16 | 17;
  earlyAlert: EarlyAlert | null;
  hasCheckbox: boolean;
  checklist: ChecklistItem[];
  startsAt: string | null;
  endsAt: string | null;
  createdAt: string | null;
  modifiedAt: string | null;
  hasLocation: boolean;
  locationAddress: string | null;
  location: LocationConfig | null;
  url: string | null;
};

export type EarlyAlert = {
  offset: number;
  unit: 'm' | 'h' | 'd' | 'w' | 'mo' | 'y';
  exactTime: number | null;
};

export type ChecklistItem = {
  text: string;
  done: boolean;
};

export type LocationConfig = {
  transitionType: number;
  latitude: number | null;
  longitude: number | null;
  address: string | null;
  placeOfInterest: string | null;
  repeatType: number;
  profileType: number;
  profileName: string | null;
  radius: number | null;
};

export type RepeatConfig = {
  unit: 'none' | 'minute' | 'hour' | 'day' | 'week' | 'month' | 'year';
  interval: number;
  count: number | null;
  until: string | null;
  rrule?: string | null;
  byDay?: string | null;
  byMonthDay?: number | null;
  byMonth?: number | null;
};

export type ReminderCategory = {
  id: string;
  name: string;
  color: number;
  iconIndex: number;
  order: number;
  extensionInfo: string | null;
};

export type CategoryList = {
  count: number;
  categories: ReminderCategory[];
};

export type ReminderList = {
  count: number;
  reminders: Reminder[];
  hasMore: boolean;
};

export type BridgeProbe = {
  extensionId?: string;
  credentialsApi: boolean;
  credentialAvailable: boolean;
  accountEmail: string | null;
  transport: string;
  reminderTableStatus: number;
  reminderRecordAvailable: boolean;
};

type Operation = 'probe' | 'list' | 'list_categories' | 'create_category' | 'update_category' | 'delete_category' | 'get' | 'create' | 'update' | 'delete';

const preview = !('__TAURI_INTERNALS__' in window);
let previewReminders: Reminder[] = [
  {
    id: 'preview-1', title: 'Pick up the parcel', text: 'Parcel shop closes at 7 PM',
    completed: false, itemStatus: 1, eventType: 4, eventStatus: 1, favorite: true, categoryId: 'LOCAL_SPACE',
    allDay: false, reminderAt: new Date(Date.now() + 45 * 60_000).toISOString(), startsAt: null,
    repeatType: 5, repeatWeekdays: 0, rrule: 'FREQ=DAILY;INTERVAL=1;WKST=SU',
    repeat: { unit: 'day', interval: 1, count: null, until: null }, tpoType: 0, soundType: 0, alertType: 16, earlyAlert: null,
    endsAt: null, createdAt: new Date().toISOString(), modifiedAt: new Date().toISOString(),
    hasCheckbox: false, checklist: [], hasLocation: false, locationAddress: null, location: null, url: null,
  },
  {
    id: 'preview-2', title: 'Water the plants', text: 'Monstera and herbs',
    completed: false, itemStatus: 1, eventType: 1, eventStatus: 1, favorite: false, categoryId: 'LOCAL_SPACE',
    allDay: false, reminderAt: new Date(Date.now() + 24 * 60 * 60_000).toISOString(), startsAt: null,
    repeatType: 0, repeatWeekdays: 0, rrule: null, repeat: null, tpoType: 0, soundType: 0, alertType: 16, earlyAlert: null,
    endsAt: null, createdAt: new Date().toISOString(), modifiedAt: new Date().toISOString(),
    hasCheckbox: false, checklist: [], hasLocation: false, locationAddress: null, location: null, url: null,
  },
  {
    id: 'preview-3', title: 'Call the dentist', text: '',
    completed: false, itemStatus: 1, eventType: 0, eventStatus: 0, favorite: false, categoryId: 'LOCAL_SPACE',
    allDay: false, reminderAt: null, startsAt: null, endsAt: null, createdAt: new Date().toISOString(),
    repeatType: 0, repeatWeekdays: 0, rrule: null, repeat: null, tpoType: 0, soundType: 0, alertType: 16, earlyAlert: null,
    modifiedAt: new Date().toISOString(), hasCheckbox: true,
    checklist: [{ text: 'Confirm appointment', done: false }, { text: 'Bring insurance card', done: true }],
    hasLocation: false, locationAddress: null, location: null, url: null,
  },
  {
    id: 'preview-4', title: 'Send the invoice', text: 'August maintenance',
    completed: true, itemStatus: 2, eventType: 0, eventStatus: 0, favorite: false, categoryId: 'LOCAL_SPACE',
    allDay: false, reminderAt: null, startsAt: null, endsAt: null, createdAt: new Date().toISOString(),
    repeatType: 0, repeatWeekdays: 0, rrule: null, repeat: null, tpoType: 0, soundType: 0, alertType: 16, earlyAlert: null,
    modifiedAt: new Date().toISOString(), hasCheckbox: false, checklist: [], hasLocation: false, locationAddress: null, location: null, url: null,
  },
];
let previewCategories: ReminderCategory[] = [
  { id: 'LOCAL_SPACE', name: 'My reminders', color: 0, iconIndex: 0, order: 0, extensionInfo: null },
  { id: 'preview-work', name: 'Work', color: 3, iconIndex: 40, order: 1, extensionInfo: null },
  { id: 'preview-study', name: 'Study', color: 4, iconIndex: 28, order: 2, extensionInfo: null },
];

export const isPreview = preview;

function reminderEventType(allDay: boolean, hasSchedule: boolean, hasLocation: boolean, repeats: boolean): number {
  if (allDay) return hasLocation ? 9 : 0;
  if (hasSchedule) {
    if (hasLocation) return 8;
    return repeats ? 4 : 1;
  }
  return hasLocation ? 5 : 0;
}

async function previewOperation<T>(operation: Operation, args: Record<string, unknown>): Promise<T> {
  await new Promise((resolve) => setTimeout(resolve, 180));
  if (operation === 'probe') {
    return {
      extensionId: 'preview',
      credentialsApi: true,
      credentialAvailable: true,
      accountEmail: 'demo@example.com',
      transport: 'direct-after-hidden-bootstrap',
      reminderTableStatus: 200,
      reminderRecordAvailable: true,
    } as T;
  }
  if (operation === 'list') {
    return { count: previewReminders.length, reminders: previewReminders, hasMore: false } as T;
  }
  if (operation === 'list_categories') {
    return { count: previewCategories.length, categories: previewCategories } as T;
  }
  if (operation === 'create_category') {
    const category: ReminderCategory = {
      id: String(args.id || crypto.randomUUID()),
      name: String(args.name).trim(),
      color: Number(args.color || 0),
      iconIndex: Number(args.iconIndex ?? 1),
      order: Number(args.order || previewCategories.length),
      extensionInfo: null,
    };
    previewCategories = [...previewCategories, category];
    return { uploadStatus: 200, category } as T;
  }
  if (operation === 'update_category') {
    let updated!: ReminderCategory;
    previewCategories = previewCategories.map((category) => {
      if (category.id !== args.id) return category;
      updated = {
        ...category,
        ...(args.name !== undefined ? { name: String(args.name).trim() } : {}),
        ...(args.color !== undefined ? { color: Number(args.color) } : {}),
        ...(args.iconIndex !== undefined ? { iconIndex: Number(args.iconIndex) } : {}),
        ...(args.order !== undefined ? { order: Number(args.order) } : {}),
      };
      return updated;
    });
    return { uploadStatus: 200, category: updated } as T;
  }
  if (operation === 'delete_category') {
    previewCategories = previewCategories.filter((category) => category.id !== args.id);
    previewReminders = previewReminders.map((reminder) => reminder.categoryId === args.id ? { ...reminder, categoryId: 'LOCAL_SPACE' } : reminder);
    return { deleted: true, id: args.id, status: 200, movedReminders: 0 } as T;
  }
  if (operation === 'get') {
    return previewReminders.find((item) => item.id === args.id) as T;
  }
  if (operation === 'create') {
    const reminder = makeOptimisticReminder(args, String(args.id || crypto.randomUUID()));
    previewReminders = [reminder, ...previewReminders];
    return { uploadStatus: 200, reminder } as T;
  }
  if (operation === 'update') {
    let updated!: Reminder;
    previewReminders = previewReminders.map((item) => {
      if (item.id !== args.id) return item;
      updated = applyReminderPatch(item, args);
      return updated;
    });
    return { uploadStatus: 200, reminder: updated } as T;
  }
  if (operation === 'delete') {
    previewReminders = previewReminders.filter((item) => item.id !== args.id);
    return { deleted: true, id: args.id, status: 200 } as T;
  }
  throw new Error('Unsupported preview operation');
}

export function makeOptimisticReminder(args: Record<string, unknown>, id: string = crypto.randomUUID()): Reminder {
  const now = new Date().toISOString();
  const reminderAt = args.reminderAt ? String(args.reminderAt) : null;
  const allDay = Boolean(args.allDay);
  const checklist = (args.checklist as ChecklistItem[] | undefined) || [];
  const location = (args.location as LocationConfig | null | undefined) || null;
  const repeat = (args.repeat as RepeatConfig | null | undefined) || null;
  return {
    id,
    title: String(args.title || 'Untitled'),
    text: String(args.text ?? ''),
    completed: Boolean(args.completed),
    itemStatus: args.completed ? 2 : 1,
    eventType: reminderEventType(allDay, Boolean(reminderAt), Boolean(location), Boolean(repeat)),
    eventStatus: reminderAt || location ? 1 : 0,
    favorite: Boolean(args.favorite),
    categoryId: String(args.categoryId || 'LOCAL_SPACE'),
    allDay,
    reminderAt: allDay ? null : reminderAt,
    startsAt: allDay ? reminderAt : null,
    endsAt: allDay && reminderAt ? new Date(new Date(reminderAt).getTime() + 86_400_000).toISOString() : null,
    repeatType: repeat ? 5 : 0,
    repeatWeekdays: 0,
    rrule: null,
    repeat,
    tpoType: 0,
    soundType: 0,
    alertType: Number(args.alertType ?? 16) as 0 | 16 | 17,
    earlyAlert: (args.earlyAlert as EarlyAlert | null | undefined) || null,
    hasCheckbox: checklist.length > 0,
    checklist,
    createdAt: now,
    modifiedAt: now,
    hasLocation: Boolean(location),
    locationAddress: location?.address || null,
    location,
    url: null,
  };
}

export function applyReminderPatch(item: Reminder, args: Record<string, unknown>): Reminder {
  const location = args.location !== undefined ? args.location as LocationConfig | null : item.location;
  const checklist = args.checklist !== undefined ? args.checklist as ChecklistItem[] : item.checklist;
  const allDay = args.allDay !== undefined ? Boolean(args.allDay) : item.allDay;
  const suppliedSchedule = args.reminderAt !== undefined
    ? args.reminderAt ? String(args.reminderAt) : null
    : item.reminderAt || item.startsAt;
  const reminderAt = allDay ? null : suppliedSchedule;
  const startsAt = allDay ? suppliedSchedule : null;
  const repeat = args.repeat !== undefined ? args.repeat as RepeatConfig | null : item.repeat;
  const completed = args.completed !== undefined ? Boolean(args.completed) : item.completed;
  return {
    ...item,
    ...(args.title !== undefined ? { title: String(args.title) } : {}),
    ...(args.text !== undefined ? { text: String(args.text) } : {}),
    completed,
    itemStatus: completed ? 2 : 1,
    ...(args.favorite !== undefined ? { favorite: Boolean(args.favorite) } : {}),
    ...(args.categoryId !== undefined ? { categoryId: String(args.categoryId) } : {}),
    allDay,
    reminderAt,
    startsAt,
    endsAt: allDay && startsAt ? new Date(new Date(startsAt).getTime() + 86_400_000).toISOString() : null,
    repeat,
    repeatType: repeat ? 5 : 0,
    ...(args.alertType !== undefined ? { alertType: Number(args.alertType) as 0 | 16 | 17 } : {}),
    ...(args.earlyAlert !== undefined ? { earlyAlert: args.earlyAlert as EarlyAlert | null } : {}),
    checklist,
    hasCheckbox: checklist.length > 0,
    location,
    hasLocation: Boolean(location),
    locationAddress: location?.address || null,
    eventType: reminderEventType(allDay, Boolean(reminderAt || startsAt), Boolean(location), Boolean(repeat)),
    eventStatus: reminderAt || startsAt || location ? 1 : 0,
    modifiedAt: new Date().toISOString(),
  };
}

export async function reminderOperation<T>(
  operation: Operation,
  args: Record<string, unknown> = {},
  endpoint = localStorage.getItem('reminder-cdp-endpoint') || 'http://127.0.0.1:9226',
): Promise<T> {
  if (preview) return previewOperation<T>(operation, args);
  return invoke<T>('reminder_operation', { operation, args, endpoint });
}

export async function clearReminderCredential(): Promise<void> {
  if (preview) return;
  return invoke<void>('clear_reminder_credential');
}
