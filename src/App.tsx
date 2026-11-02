import { t, useI18n, getLocale, formatNumber } from './i18n';
import { FormEvent, useEffect, useMemo, useRef, useState } from 'react';
import { useLiveQuery } from '@tanstack/react-db';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import * as AlertDialog from '@radix-ui/react-alert-dialog';
import * as Dialog from '@radix-ui/react-dialog';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import * as Popover from '@radix-ui/react-popover';
import * as ScrollArea from '@radix-ui/react-scroll-area';
import * as Separator from '@radix-ui/react-separator';
import * as Switch from '@radix-ui/react-switch';
import * as Tooltip from '@radix-ui/react-tooltip';
import {
  Bell,
  BellRing,
  CalendarDays,
  Check,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleUserRound,
  Circle,
  Clock3,
  EllipsisVertical,
  FolderCog,
  LoaderCircle,
  LogIn,
  LockKeyhole,
  MapPin,
  Mic,
  Moon,
  Plus,
  RefreshCw,
  Repeat2,
  Search,
  Save,
  Settings2,
  Share2,
  Star,
  Sun,
  Trash2,
  TriangleAlert,
  Volume2,
  X,
} from 'lucide-react';
import { categoryColor, categoryColorName, categoryColorOrder, categoryIcon, categoryIcons } from './lib/category-icons';
import { createReminderCollections } from './lib/reminder-collections';
import {
  applyReminderPatch,
  clearReminderCredential,
  isPreview,
  makeOptimisticReminder,
  reminderOperation,
  type BridgeProbe,
  type ChecklistItem,
  type EarlyAlert,
  type LocationConfig,
  type Reminder,
  type ReminderCategory,
  type RepeatConfig,
} from './lib/reminders';

type CategoryId = string;
type Theme = 'light' | 'dark';
type Notice = { message: string; kind: 'success' | 'error' };
type PersistableTransaction = { isPersisted: { promise: Promise<unknown> } };
type ConsentState = 'pending' | 'accepted' | 'declined';
type SettingsDialogProps = {
  endpoint: string;
  setEndpoint: (value: string) => void;
  theme: Theme;
  setTheme: (value: Theme) => void;
  connected: boolean;
  onDisconnect: () => Promise<void>;
  onWipe: () => Promise<void>;
};

const consentStorageKey = 'samsung-reminder-cloud-consent-v1';

function hasStoredConsent() {
  try {
    return localStorage.getItem(consentStorageKey) === 'accepted';
  } catch {
    return false;
  }
}

function getCategoryDefinitions() { return [
  { id: 'today' as const, label: t("ui.today"), icon: CalendarDays, color: '#ff6f7d', wash: '#fff0f2' },
  { id: 'scheduled' as const, label: t("ui.scheduled"), icon: Clock3, color: '#43cbd0', wash: '#eafafa' },
  { id: 'important' as const, label: t("ui.important"), icon: Star, color: '#ffc928', wash: '#fff8df' },
  { id: 'place' as const, label: t("ui.place"), icon: MapPin, color: '#59aef2', wash: '#edf7ff' },
  { id: 'no-alert' as const, label: t("ui.no_alert"), icon: Bell, color: '#8e6bea', wash: '#f3efff' },
  { id: 'completed' as const, label: t("ui.completed"), icon: CheckCircle2, color: '#94979a', wash: '#f1f2f3' },
]; }

function sameLocalDay(iso: string | null) {
  if (!iso) return false;
  const date = new Date(iso);
  const now = new Date();
  return date.getFullYear() === now.getFullYear()
    && date.getMonth() === now.getMonth()
    && date.getDate() === now.getDate();
}

function isInCategory(reminder: Reminder, category: CategoryId) {
  if (category === 'all') return !reminder.completed;
  if (category === 'today') return !reminder.completed && sameLocalDay(reminder.reminderAt || reminder.startsAt);
  if (category === 'scheduled') return !reminder.completed && Boolean(reminder.reminderAt || reminder.startsAt);
  if (category === 'important') return !reminder.completed && reminder.favorite;
  if (category === 'place') return !reminder.completed && reminder.hasLocation;
  if (category === 'no-alert') return !reminder.completed && !reminder.reminderAt && !reminder.startsAt && !reminder.hasLocation;
  if (category.startsWith('category:')) return !reminder.completed && reminder.categoryId === category.slice('category:'.length);
  return reminder.completed;
}

function toDateTimeLocal(value: string | null) {
  if (!value) return '';
  const date = new Date(value);
  const offset = date.getTimezoneOffset() * 60_000;
  return new Date(date.getTime() - offset).toISOString().slice(0, 16);
}

function toAllDayInput(value: string | null) {
  return value ? value.slice(0, 10) : '';
}

function scheduleInput(reminder: Reminder | null) {
  if (!reminder) return '';
  return reminder.allDay
    ? toAllDayInput(reminder.startsAt || reminder.reminderAt)
    : toDateTimeLocal(reminder.reminderAt || reminder.startsAt);
}

function errorMessage(error: unknown) {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  return t("ui.samsung_reminder_could_not_connect");
}

function connectionIssue(error: unknown) {
  const message = errorMessage(error);
  if (message.includes('SAMSUNG_BROWSER_NOT_FOUND')) {
    return {
      title: t("ui.samsung_browser_was_not_found"),
      detail: t("ui.install_samsung_browser_for_windows_in_its_standard_location_open_it_once_then_retry"),
      action: t("ui.check_again"),
      icon: TriangleAlert,
    };
  }
  if (message.includes('SAMSUNG_PROFILE_NOT_READY')) {
    return {
      title: t("ui.samsung_browser_needs_setup"),
      detail: t("ui.open_samsung_browser_once_to_initialize_its_profile_close_it_then_retry_here"),
      action: t("ui.i_opened_it_retry"),
      icon: TriangleAlert,
    };
  }
  if (message.includes('SAMSUNG_ACCOUNT_NOT_SIGNED_IN')) {
    return {
      title: t("ui.samsung_account_sign_in_required"),
      detail: t("ui.sign_in_to_your_samsung_account_in_samsung_browser_close_the_browser_then_retry"),
      action: t("ui.i_signed_in_retry"),
      icon: LogIn,
    };
  }
  if (message.includes('SAMSUNG_BROWSER_BUSY')) {
    return {
      title: t("ui.samsung_browser_is_already_in_use"),
      detail: t("ui.close_samsung_browser_completely_so_the_hidden_sync_helper_can_use_its_signed_in_profile_then_retry"),
      action: t("ui.i_closed_it_retry"),
      icon: TriangleAlert,
    };
  }
  return {
    title: t("ui.samsung_sync_unavailable"),
    detail: t("error.syncDetail"),
    action: t("ui.retry_sync"),
    icon: TriangleAlert,
  };
}

function padNumber(value: number) {
  return String(value).padStart(2, '0');
}

function minutesToClock(value: number | null) {
  const minutes = value == null ? 9 * 60 : Math.max(0, Math.min(1439, value));
  return `${padNumber(Math.floor(minutes / 60))}:${padNumber(minutes % 60)}`;
}

function clockToMinutes(value: string) {
  const [hours = 9, minutes = 0] = value.split(':').map(Number);
  return Math.max(0, Math.min(1439, hours * 60 + minutes));
}

function earlyAlertPreset(value: EarlyAlert | null | undefined): 'none' | 'day' | 'week' | 'custom' {
  if (!value) return 'none';
  if (value.offset === 1 && value.unit === 'd' && value.exactTime == null) return 'day';
  if (value.offset === 1 && value.unit === 'w' && value.exactTime == null) return 'week';
  return 'custom';
}

function pickerValue(date: Date, includeTime: boolean) {
  const day = `${date.getFullYear()}-${padNumber(date.getMonth() + 1)}-${padNumber(date.getDate())}`;
  return includeTime ? `${day}T${padNumber(date.getHours())}:${padNumber(date.getMinutes())}` : day;
}

function pickerDate(value: string, includeTime: boolean) {
  if (!value) return null;
  const parsed = new Date(includeTime ? value : `${value}T12:00:00`);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

type OneUiSelectOption = {
  value: string;
  label: string;
  icon?: React.ComponentType<{ size?: number; strokeWidth?: number }>;
  color?: string;
};

function SelectOptionContent({ option }: { option: OneUiSelectOption }) {
  const Icon = option.icon;
  return (
    <span className="one-ui-select-value">
      {Icon ? (
        <span className="one-ui-select-option-icon" style={{ '--option-color': option.color } as React.CSSProperties}>
          <Icon size={14} strokeWidth={2.5} />
        </span>
      ) : null}
      <span>{option.label}</span>
    </span>
  );
}

function OneUiSelect({ value, options, onChange, ariaLabel, className = '', disabled = false }: {
  value: string;
  options: OneUiSelectOption[];
  onChange: (value: string) => void;
  ariaLabel: string;
  className?: string;
  disabled?: boolean;
}) {
  const selected = options.find((option) => option.value === value);

  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger
        type="button"
        className={`one-ui-select ${className}`}
        aria-label={ariaLabel}
        disabled={disabled}
      >
        {selected ? <SelectOptionContent option={selected} /> : <span>{value}</span>}
        <ChevronDown size={14} aria-hidden="true" />
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          className="one-ui-select-content"
          align="start"
          sideOffset={6}
          collisionPadding={12}
        >
          <DropdownMenu.RadioGroup value={value} onValueChange={onChange}>
            {options.map((option) => (
              <DropdownMenu.RadioItem key={option.value} value={option.value} className="one-ui-select-item">
                <SelectOptionContent option={option} />
                <DropdownMenu.ItemIndicator className="one-ui-select-indicator">
                  <Check size={14} strokeWidth={2.6} />
                </DropdownMenu.ItemIndicator>
              </DropdownMenu.RadioItem>
            ))}
          </DropdownMenu.RadioGroup>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

function OneUiDatePicker({ value, onChange, includeTime = true, disabled = false }: {
  value: string;
  onChange: (value: string) => void;
  includeTime?: boolean;
  disabled?: boolean;
}) {
  useI18n();
  const selected = pickerDate(value, includeTime);
  const [open, setOpen] = useState(false);
  const [viewMonth, setViewMonth] = useState(() => {
    const initial = selected || new Date();
    return new Date(initial.getFullYear(), initial.getMonth(), 1);
  });

  useEffect(() => {
    if (!selected) return;
    setViewMonth(new Date(selected.getFullYear(), selected.getMonth(), 1));
  }, [value]);

  const calendarDays = useMemo(() => {
    const firstWeekday = (viewMonth.getDay() + 6) % 7;
    const first = new Date(viewMonth.getFullYear(), viewMonth.getMonth(), 1 - firstWeekday);
    return Array.from({ length: 42 }, (_, index) => new Date(first.getFullYear(), first.getMonth(), first.getDate() + index));
  }, [viewMonth]);
  const display = selected
    ? new Intl.DateTimeFormat(getLocale(), includeTime
      ? { weekday: 'short', month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' }
      : { weekday: 'short', month: 'short', day: 'numeric', year: 'numeric' }).format(selected)
    : includeTime ? t("ui.set_date_time") : t("ui.choose_date");

  function chooseDay(day: Date) {
    const next = selected ? new Date(selected) : new Date();
    next.setFullYear(day.getFullYear(), day.getMonth(), day.getDate());
    if (!selected && includeTime) {
      next.setHours(next.getHours() + 1, 0, 0, 0);
    }
    onChange(pickerValue(next, includeTime));
  }

  function setTime(part: 'hour' | 'minute', number: number) {
    const next = selected ? new Date(selected) : new Date();
    if (part === 'hour') next.setHours(number);
    else next.setMinutes(number);
    next.setSeconds(0, 0);
    onChange(pickerValue(next, true));
  }

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <button type="button" disabled={disabled} className="date-picker-trigger">
          <span className={selected ? 'text-ink' : 'text-muted'}>{display}</span>
          <ChevronRight size={15} className="text-muted" />
        </button>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content side="left" align="start" sideOffset={18} collisionPadding={16} className="date-picker-popover">
          <div className="date-picker-header">
            <button type="button" aria-label={t("ui.previous_month")} onClick={() => setViewMonth(new Date(viewMonth.getFullYear(), viewMonth.getMonth() - 1, 1))}><ChevronLeft size={18} /></button>
            <strong>{new Intl.DateTimeFormat(getLocale(), { month: 'long', year: 'numeric' }).format(viewMonth)}</strong>
            <button type="button" aria-label={t("ui.next_month")} onClick={() => setViewMonth(new Date(viewMonth.getFullYear(), viewMonth.getMonth() + 1, 1))}><ChevronRight size={18} /></button>
          </div>
          <div className="calendar-weekdays">{Array.from({ length: 7 }, (_, index) => <span key={index}>{new Intl.DateTimeFormat(getLocale(), { weekday: 'short' }).format(new Date(2026, 10, 2 + index))}</span>)}</div>
          <div className="calendar-grid">
            {calendarDays.map((day) => {
              const isSelected = selected && day.toDateString() === selected.toDateString();
              const isToday = day.toDateString() === new Date().toDateString();
              const outside = day.getMonth() !== viewMonth.getMonth();
              return (
                <button
                  type="button"
                  key={day.toISOString()}
                  aria-label={new Intl.DateTimeFormat(getLocale(), { dateStyle: 'full' }).format(day)}
                  className={`${isSelected ? 'is-selected' : ''} ${isToday ? 'is-today' : ''} ${outside ? 'is-outside' : ''}`}
                  onClick={() => chooseDay(day)}
                >
                  {formatNumber(day.getDate())}
                </button>
              );
            })}
          </div>
          {includeTime ? (
            <div className="date-picker-time">
              <Clock3 size={17} />
              <span>{t("ui.time")}</span>
              <OneUiSelect
                ariaLabel={t("ui.hour")}
                value={String(selected?.getHours() ?? new Date().getHours())}
                onChange={(next) => setTime('hour', Number(next))}
                options={Array.from({ length: 24 }, (_, hour) => ({ value: String(hour), label: padNumber(hour) }))}
              />
              <span>:</span>
              <OneUiSelect
                ariaLabel={t("ui.minute")}
                value={String(selected?.getMinutes() ?? 0)}
                onChange={(next) => setTime('minute', Number(next))}
                options={Array.from({ length: 60 }, (_, minute) => ({ value: String(minute), label: padNumber(minute) }))}
              />
            </div>
          ) : null}
          <div className="date-picker-actions">
            <button type="button" onClick={() => onChange('')}>{t("ui.clear")}</button>
            <button type="button" onClick={() => { onChange(pickerValue(new Date(), includeTime)); setViewMonth(new Date(new Date().getFullYear(), new Date().getMonth(), 1)); }}>{t("ui.today")}</button>
            <button type="button" className="is-primary" onClick={() => setOpen(false)}>{t("ui.done")}</button>
          </div>
          <Popover.Arrow className="date-picker-arrow" />
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

function formatRepeat(repeat: RepeatConfig | null) {
  if (!repeat || repeat.unit === 'none') return t("ui.does_not_repeat");
  let label = t(`repeat.every.${repeat.unit}`, { count: repeat.interval });
  if (repeat.count) label += t("repeat.count", { count: repeat.count });
  if (repeat.until) label += t('repeat.until', { date: new Intl.DateTimeFormat(getLocale(), { dateStyle: 'medium' }).format(new Date(repeat.until)) });
  return label;
}

function reminderGroupLabel(reminder: Reminder) {
  const value = reminder.reminderAt || reminder.startsAt;
  if (!value) return reminder.hasLocation ? t("ui.place") : t("ui.no_alert");
  const date = new Date(value);
  const now = new Date();
  if (date.getFullYear() === now.getFullYear()) {
    return new Intl.DateTimeFormat(getLocale(), { month: 'short' }).format(date);
  }
  return new Intl.DateTimeFormat(getLocale(), { month: 'short', year: 'numeric' }).format(date);
}

function categoryVisual(category: ReminderCategory) {
  return {
    icon: categoryIcon(category.iconIndex).icon,
    color: categoryColor(category.color),
    wash: '#282233',
  };
}

function formatWhen(reminder: Reminder) {
  const value = reminder.reminderAt || reminder.startsAt;
  if (!value) return reminder.hasLocation ? reminder.locationAddress || t("ui.location_reminder") : t("ui.no_alert");
  const date = new Date(value);
  if (reminder.allDay) {
    return t('schedule.allDay', { date: new Intl.DateTimeFormat(getLocale(), { weekday: 'short', month: 'short', day: 'numeric' }).format(date) });
  }
  const today = sameLocalDay(value);
  return new Intl.DateTimeFormat(getLocale(), today
    ? { hour: 'numeric', minute: '2-digit' }
    : { weekday: 'short', month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' })
    .format(date);
}

function IconButton({ label, children, onClick, className = '' }: {
  label: string;
  children: React.ReactNode;
  onClick?: () => void;
  className?: string;
}) {
  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>
        <button type="button" aria-label={label} onClick={onClick} className={`icon-button ${className}`}>
          {children}
        </button>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content sideOffset={8} className="tooltip-content">{label}</Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}

function CategoryIconPicker({ value, color, label, disabled = false, onChange }: {
  value: number;
  color: number;
  label: string;
  disabled?: boolean;
  onChange: (value: number) => void;
}) {
  useI18n();
  const [open, setOpen] = useState(false);
  const selected = categoryIcon(value);
  const SelectedIcon = selected.icon;

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <button
          type="button"
          className="category-icon-trigger"
          style={{ '--swatch': categoryColor(color) } as React.CSSProperties}
          aria-label={label}
          disabled={disabled}
        >
          <SelectedIcon size={19} strokeWidth={2.4} />
        </button>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content
          side="right"
          align="center"
          sideOffset={12}
          collisionPadding={16}
          className="category-icon-popover"
        >
          <div className="category-icon-popover-heading">
            <strong>{t("ui.choose_icon")}</strong>
            <span>{selected.label}</span>
          </div>
          <div className="category-icon-grid">
            {categoryIcons.map((definition, index) => {
              const Icon = definition.icon;
              return (
                <button
                  type="button"
                  key={definition.label}
                  aria-label={definition.label}
                  aria-pressed={value === index}
                  onClick={() => {
                    onChange(index);
                    setOpen(false);
                  }}
                >
                  <Icon size={19} strokeWidth={2.25} />
                </button>
              );
            })}
          </div>
          <Popover.Arrow className="category-icon-popover-arrow" />
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

function CategoryCard({ definition, count, selected, onSelect }: {
  definition: { id: string; label: string; icon: React.ComponentType<{ size?: number; strokeWidth?: number; fill?: string }>; color: string; wash: string };
  count: number;
  selected: boolean;
  onSelect: () => void;
}) {
  useI18n();
  const Icon = definition.icon;
  return (
    <button
      type="button"
      onClick={onSelect}
      aria-pressed={selected}
      className="category-card group"
      style={{ '--category': definition.color, '--category-wash': definition.wash } as React.CSSProperties}
    >
      <span className="category-icon"><Icon size={21} strokeWidth={2.5} fill={definition.id === 'important' ? 'currentColor' : 'none'} /></span>
      <span className="mt-auto flex w-full items-end justify-between gap-2">
        <span className="text-[14px] font-medium text-ink/66">{definition.label}</span>
        <span className="text-[20px] font-semibold tracking-[-0.04em] text-ink">{formatNumber(count)}</span>
      </span>
    </button>
  );
}

function ReminderRow({ reminder, selected, onSelect, onToggle }: {
  reminder: Reminder;
  selected: boolean;
  onSelect: () => void;
  onToggle: () => void;
}) {
  useI18n();
  return (
    <article className={`reminder-row ${selected ? 'is-selected' : ''}`}>
      <button
        type="button"
        className="completion-button"
        aria-label={reminder.completed ? t("ui.mark_incomplete") : t("ui.mark_complete")}
        onClick={(event) => { event.stopPropagation(); onToggle(); }}
      >
        {reminder.completed ? <Check size={17} strokeWidth={3} /> : null}
      </button>
      <button type="button" onClick={onSelect} className="min-w-0 flex-1 text-left">
        <span className={`block truncate text-[16px] font-medium tracking-[-0.012em] ${reminder.completed ? 'text-muted line-through' : 'text-ink'}`}>
          {reminder.title}
        </span>
        <span className="mt-1 flex items-center gap-1.5 truncate text-[12px] text-muted">
          {reminder.hasLocation ? <MapPin size={13} /> : reminder.allDay ? <CalendarDays size={13} /> : reminder.reminderAt || reminder.startsAt ? <Clock3 size={13} /> : <Bell size={13} />}
          {formatWhen(reminder)}
          {reminder.text ? <><span aria-hidden>·</span><span className="truncate">{reminder.text}</span></> : null}
          {reminder.checklist.length ? <><span aria-hidden>·</span><span>{t("checklist.progress", { done: formatNumber(reminder.checklist.filter((item) => item.done).length), total: formatNumber(reminder.checklist.length) })}</span></> : null}
        </span>
        {reminder.repeat ? (
          <span className="mt-1 flex items-center gap-1.5 truncate text-[12px] text-muted">
            <Repeat2 size={13} /> {formatRepeat(reminder.repeat)}
          </span>
        ) : null}
      </button>
      {reminder.favorite ? <Star size={17} fill="currentColor" className="shrink-0 text-[#ffc928]" /> : null}
      <ChevronRight size={17} className="shrink-0 text-muted/50" />
    </article>
  );
}

function Inspector({ reminder, categories, onSave, onDelete, saving, open, onClose }: {
  reminder: Reminder | null;
  categories: ReminderCategory[];
  onSave: (values: Record<string, unknown>) => void;
  onDelete: (id: string) => void;
  saving: boolean;
  open: boolean;
  onClose: () => void;
}) {
  useI18n();
  const [title, setTitle] = useState('');
  const [text, setText] = useState('');
  const [checklist, setChecklist] = useState<ChecklistItem[]>([]);
  const [newChecklistText, setNewChecklistText] = useState('');
  const checklistInputRef = useRef<HTMLInputElement>(null);
  const [completed, setCompleted] = useState(false);
  const [favorite, setFavorite] = useState(false);
  const [scheduledAt, setScheduledAt] = useState('');
  const [allDay, setAllDay] = useState(false);
  const [repeatUnit, setRepeatUnit] = useState<RepeatConfig['unit']>('none');
  const [repeatInterval, setRepeatInterval] = useState(1);
  const [repeatEnd, setRepeatEnd] = useState<'forever' | 'count' | 'until'>('forever');
  const [repeatCount, setRepeatCount] = useState(10);
  const [repeatUntil, setRepeatUntil] = useState('');
  const [repeatWeekday, setRepeatWeekday] = useState('MO');
  const [repeatMonthDay, setRepeatMonthDay] = useState(1);
  const [repeatMonth, setRepeatMonth] = useState(1);
  const [alertType, setAlertType] = useState<0 | 16 | 17>(16);
  const [earlyPreset, setEarlyPreset] = useState<'none' | 'day' | 'week' | 'custom'>('none');
  const [earlyOffset, setEarlyOffset] = useState(1);
  const [earlyUnit, setEarlyUnit] = useState<EarlyAlert['unit']>('d');
  const [earlyTime, setEarlyTime] = useState('09:00');
  const [categoryId, setCategoryId] = useState('LOCAL_SPACE');
  const [locationEnabled, setLocationEnabled] = useState(false);
  const [locationAddress, setLocationAddress] = useState('');
  const [locationLatitude, setLocationLatitude] = useState('');
  const [locationLongitude, setLocationLongitude] = useState('');
  const [locationTransition, setLocationTransition] = useState(1);
  const [locationRadius, setLocationRadius] = useState(200);

  useEffect(() => {
    setTitle(reminder?.title || '');
    setText(reminder?.text || '');
    setChecklist(reminder?.checklist.map((item) => ({ ...item })) || []);
    setNewChecklistText('');
    setCompleted(Boolean(reminder?.completed));
    setFavorite(Boolean(reminder?.favorite));
    setScheduledAt(scheduleInput(reminder));
    setAllDay(Boolean(reminder?.allDay));
    setRepeatUnit(reminder?.repeat?.unit || 'none');
    setRepeatInterval(reminder?.repeat?.interval || 1);
    setRepeatCount(reminder?.repeat?.count || 10);
    setRepeatUntil(reminder?.repeat?.until?.slice(0, 10) || '');
    setRepeatEnd(reminder?.repeat?.count ? 'count' : reminder?.repeat?.until ? 'until' : 'forever');
    const initialDate = pickerDate(scheduleInput(reminder), !reminder?.allDay) || new Date();
    const weekDays = ['SU', 'MO', 'TU', 'WE', 'TH', 'FR', 'SA'];
    setRepeatWeekday(reminder?.repeat?.byDay || weekDays[initialDate.getDay()]);
    setRepeatMonthDay(reminder?.repeat?.byMonthDay || initialDate.getDate());
    setRepeatMonth(reminder?.repeat?.byMonth || initialDate.getMonth() + 1);
    setAlertType(reminder?.alertType ?? 16);
    const early = reminder?.earlyAlert;
    setEarlyPreset(earlyAlertPreset(early));
    setEarlyOffset(early?.offset || 1);
    setEarlyUnit(early?.unit || 'd');
    setEarlyTime(minutesToClock(early?.exactTime ?? null));
    setCategoryId(reminder?.categoryId || 'LOCAL_SPACE');
    setLocationEnabled(Boolean(reminder?.location));
    setLocationAddress(reminder?.location?.address || '');
    setLocationLatitude(reminder?.location?.latitude?.toString() || '');
    setLocationLongitude(reminder?.location?.longitude?.toString() || '');
    setLocationTransition(reminder?.location?.transitionType || 1);
    setLocationRadius(reminder?.location?.radius || 200);
  }, [reminder]);

  if (!reminder) {
    return (
      <aside className="inspector hidden lg:flex">
        <div className="m-auto max-w-[250px] text-center">
          <div className="mx-auto mb-5 grid size-16 place-items-center rounded-[22px] bg-violet-soft text-violet">
            <Bell size={28} strokeWidth={2.2} />
          </div>
          <h2 className="text-[19px] font-semibold text-ink">{t("ui.choose_a_reminder")}</h2>
          <p className="mt-2 text-sm leading-6 text-muted">{t("ui.select_an_item_to_see_its_notes_and_make_changes")}</p>
        </div>
      </aside>
    );
  }

  const baselineEnd = reminder.repeat?.count ? 'count' : reminder.repeat?.until ? 'until' : 'forever';
  const baselineSchedule = pickerDate(scheduleInput(reminder), !reminder.allDay) || new Date();
  const normalizedChecklist = checklist
    .map((item) => ({ text: item.text.trim(), done: item.done }))
    .filter((item) => item.text);
  const locationValue: LocationConfig | null = locationEnabled ? {
    transitionType: locationTransition,
    latitude: Number.parseFloat(locationLatitude),
    longitude: Number.parseFloat(locationLongitude),
    address: locationAddress.trim() || null,
    placeOfInterest: null,
    repeatType: 10,
    profileType: 0,
    profileName: null,
    radius: Math.max(50, locationRadius),
  } : null;
  const locationValid = !locationEnabled
    || (Number.isFinite(locationValue?.latitude ?? Number.NaN) && Number.isFinite(locationValue?.longitude ?? Number.NaN));
  const earlyAlertValue = (): EarlyAlert | null => {
    if (!scheduledAt || earlyPreset === 'none') return null;
    if (earlyPreset === 'day') return { offset: 1, unit: 'd', exactTime: null };
    if (earlyPreset === 'week') return { offset: 1, unit: 'w', exactTime: null };
    return {
      offset: Math.max(1, earlyOffset),
      unit: earlyUnit,
      exactTime: allDay ? clockToMinutes(earlyTime) : null,
    };
  };
  const checklistDirty = JSON.stringify(normalizedChecklist) !== JSON.stringify(reminder.checklist);
  const locationDirty = JSON.stringify(locationValue) !== JSON.stringify(reminder.location);
  const earlyAlertDirty = JSON.stringify(earlyAlertValue()) !== JSON.stringify(reminder.earlyAlert);
  const dirty = title !== reminder.title || text !== reminder.text
    || checklistDirty || locationDirty || earlyAlertDirty
    || completed !== reminder.completed || favorite !== reminder.favorite
    || allDay !== reminder.allDay
    || scheduledAt !== scheduleInput(reminder)
    || repeatUnit !== (reminder.repeat?.unit || 'none')
    || (repeatUnit !== 'none' && repeatInterval !== (reminder.repeat?.interval || 1))
    || repeatEnd !== baselineEnd
    || (repeatEnd === 'count' && repeatCount !== (reminder.repeat?.count || 10))
    || (repeatEnd === 'until' && repeatUntil !== (reminder.repeat?.until?.slice(0, 10) || ''))
    || (repeatUnit === 'week' && repeatWeekday !== (reminder.repeat?.byDay || ['SU', 'MO', 'TU', 'WE', 'TH', 'FR', 'SA'][baselineSchedule.getDay()]))
    || ((repeatUnit === 'month' || repeatUnit === 'year') && repeatMonthDay !== (reminder.repeat?.byMonthDay || baselineSchedule.getDate()))
    || (repeatUnit === 'year' && repeatMonth !== (reminder.repeat?.byMonth || baselineSchedule.getMonth() + 1))
    || alertType !== reminder.alertType || categoryId !== (reminder.categoryId || 'LOCAL_SPACE');

  const selectedCategory = categories.find((category) => category.id === categoryId)
    || { id: 'LOCAL_SPACE', name: t("ui.my_reminders"), color: 0, iconIndex: 0, order: -1, extensionInfo: null };
  const selectedCategoryVisual = categoryVisual(selectedCategory);
  const SelectedCategoryIcon = selectedCategoryVisual.icon;

  function repeatValue(): RepeatConfig | null {
    if (!scheduledAt || repeatUnit === 'none') return null;
    const value: RepeatConfig = { unit: repeatUnit, interval: Math.max(1, repeatInterval), count: null, until: null };
    if (repeatUnit === 'week') value.byDay = repeatWeekday;
    if (repeatUnit === 'month' || repeatUnit === 'year') value.byMonthDay = repeatMonthDay;
    if (repeatUnit === 'year') value.byMonth = repeatMonth;
    if (repeatEnd === 'count') value.count = Math.max(1, repeatCount);
    if (repeatEnd === 'until' && repeatUntil) value.until = new Date(`${repeatUntil}T23:59:59`).toISOString();
    return value;
  }

  function addChecklistItem() {
    const item = newChecklistText.trim();
    if (!item) {
      checklistInputRef.current?.focus();
      return;
    }
    setChecklist((items) => [...items, { text: item, done: false }]);
    setNewChecklistText('');
    requestAnimationFrame(() => checklistInputRef.current?.focus());
  }

  async function shareReminder() {
    const schedule = scheduledAt
      ? `\n${allDay ? new Date(`${scheduledAt}T12:00:00`).toLocaleDateString(getLocale()) : new Date(scheduledAt).toLocaleString(getLocale())}`
      : '';
    const checks = normalizedChecklist.length ? `\n${normalizedChecklist.map((item) => `${item.done ? '✓' : '○'} ${item.text}`).join('\n')}` : '';
    const place = locationEnabled && locationAddress ? `\n${locationAddress}` : '';
    const body = `${title}${text ? `\n${text}` : ''}${checks}${schedule}${place}`;
    if (navigator.share) await navigator.share({ title, text: body });
    else await navigator.clipboard.writeText(body);
  }

  return (
    <aside className={`inspector hidden lg:flex ${open ? 'is-drawer' : ''}`}>
      <div className="flex items-center justify-between">
        <span className="text-xs font-semibold uppercase tracking-[0.11em] text-muted">{t("ui.details")}</span>
        <div className="flex items-center gap-1">
          <button
            type="button"
            className={`favorite-button ${favorite ? 'is-active' : ''}`}
            onClick={() => setFavorite((value) => !value)}
            aria-label={favorite ? t("ui.remove_from_important") : t("ui.mark_important")}
          >
            <Star size={19} fill={favorite ? 'currentColor' : 'none'} />
          </button>
          <button type="button" className="icon-button drawer-close" aria-label={t("ui.close_details")} onClick={onClose}>
            <X size={18} />
          </button>
        </div>
      </div>

      <div className="mt-8 flex min-h-0 flex-1 flex-col overflow-y-auto pr-2">
        <div className="flex items-start gap-4">
          <button
            type="button"
            className={`inspector-check ${completed ? 'is-complete' : ''}`}
            onClick={() => setCompleted((value) => !value)}
            aria-label={completed ? t("ui.mark_incomplete") : t("ui.mark_complete")}
          >
            {completed ? <Check size={18} strokeWidth={3} /> : null}
          </button>
          <textarea
            className={`title-editor ${completed ? 'line-through opacity-55' : ''}`}
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            rows={2}
            aria-label={t("ui.reminder_title")}
          />
        </div>
        <Separator.Root className="my-7 h-px bg-line" />
        <div className="flex items-center justify-between">
          <label className="text-xs font-semibold uppercase tracking-[0.11em] text-muted" htmlFor="notes">{t("ui.notes")}</label>
          <button type="button" className="text-button" onClick={() => checklistInputRef.current?.focus()}>
            <Plus size={14} /> {t("ui.add_to_do")}
          </button>
        </div>
        <textarea
          id="notes"
          value={text}
          onChange={(event) => setText(event.target.value)}
          placeholder={t("ui.add_notes")}
          className="notes-editor mt-3 min-h-36 resize-none rounded-[20px] bg-canvas px-4 py-3.5 text-sm leading-6 text-ink placeholder:text-muted/60"
        />
        <div className="checklist-editor">
          {checklist.length ? (
            <div className="checklist-items">
            {checklist.map((item, index) => (
              <div className="checklist-editor-row" key={index}>
                <button
                  type="button"
                  className={item.done ? 'is-done' : ''}
                  aria-label={item.done ? t("ui.mark_checklist_item_incomplete") : t("ui.mark_checklist_item_complete")}
                  onClick={() => setChecklist((items) => items.map((entry, itemIndex) => itemIndex === index ? { ...entry, done: !entry.done } : entry))}
                >
                  {item.done ? <Check size={13} strokeWidth={3} /> : null}
                </button>
                <input
                  value={item.text}
                  className={item.done ? 'is-done' : ''}
                  placeholder={t("ui.checklist_item")}
                  onChange={(event) => setChecklist((items) => items.map((entry, itemIndex) => itemIndex === index ? { ...entry, text: event.target.value } : entry))}
                />
                <button type="button" aria-label={t("ui.remove_checklist_item")} onClick={() => setChecklist((items) => items.filter((_, itemIndex) => itemIndex !== index))}><X size={14} /></button>
              </div>
            ))}
            </div>
          ) : <p className="checklist-empty">{t("ui.no_to_do_items_yet")}</p>}
          <div className="checklist-composer">
            <Plus size={16} />
            <input ref={checklistInputRef} value={newChecklistText} onChange={(event) => setNewChecklistText(event.target.value)} onKeyDown={(event) => { if (event.key === 'Enter') { event.preventDefault(); addChecklistItem(); } }} placeholder={t("ui.add_a_to_do_item")} aria-label={t("ui.new_to_do_item")} />
            <button type="button" disabled={!newChecklistText.trim()} onClick={addChecklistItem}>{t("ui.add")}</button>
          </div>
        </div>
        <div className="mt-5 space-y-3">
          <div className="detail-control items-start">
            <CalendarDays size={18} className="mt-2 text-scheduled" />
            <div className="min-w-0 flex-1">
              <div className="flex items-center justify-between gap-3">
                <span className="detail-control-label">{repeatUnit === 'none' ? (allDay ? t("ui.date") : t("ui.date_time")) : t("ui.first_reminder")}</span>
                <label className="all-day-toggle">
                  <span>{t("ui.all_day")}</span>
                  <Switch.Root
                    className="switch-root"
                    checked={allDay}
                    onCheckedChange={(checked) => {
                      setAllDay(checked);
                      if (scheduledAt) {
                        setScheduledAt(checked ? scheduledAt.slice(0, 10) : `${scheduledAt.slice(0, 10)}T09:00`);
                      }
                      if (checked && (repeatUnit === 'minute' || repeatUnit === 'hour')) setRepeatUnit('day');
                    }}
                    aria-label={t("ui.all_day_reminder")}
                  >
                    <Switch.Thumb className="switch-thumb" />
                  </Switch.Root>
                </label>
              </div>
              <OneUiDatePicker
                value={scheduledAt}
                includeTime={!allDay}
                onChange={(value) => {
                  setScheduledAt(value);
                  if (!value) {
                    setRepeatUnit('none');
                    setEarlyPreset('none');
                  }
                }}
              />
            </div>
          </div>

          <div className="detail-control items-start">
            <BellRing size={18} className="mt-2 text-violet" />
            <div className="min-w-0 flex-1">
              <span className="detail-control-label">{t("ui.early_alert")}</span>
              <OneUiSelect
                className="early-alert-preset mt-2"
                ariaLabel={t("ui.early_alert")}
                value={earlyPreset}
                disabled={!scheduledAt}
                onChange={(next) => setEarlyPreset(next as typeof earlyPreset)}
                options={[
                  { value: 'none', label: t("ui.no_early_alert") },
                  { value: 'day', label: t("ui.1_day_before") },
                  { value: 'week', label: t("ui.1_week_before") },
                  { value: 'custom', label: t("ui.custom") },
                ]}
              />
              {earlyPreset === 'custom' && scheduledAt ? (
                <div className="early-alert-editor">
                  <input
                    type="number"
                    min={1}
                    max={999}
                    value={earlyOffset}
                    onChange={(event) => setEarlyOffset(Number(event.target.value) || 1)}
                    aria-label={t("ui.early_alert_offset")}
                  />
                  <OneUiSelect
                    ariaLabel={t("ui.early_alert_unit")}
                    value={earlyUnit}
                    onChange={(next) => setEarlyUnit(next as EarlyAlert['unit'])}
                    options={[
                      { value: 'm', label: t("early.unit.m", { count: earlyOffset }) },
                      { value: 'h', label: t("early.unit.h", { count: earlyOffset }) },
                      { value: 'd', label: t("early.unit.d", { count: earlyOffset }) },
                      { value: 'w', label: t("early.unit.w", { count: earlyOffset }) },
                      { value: 'mo', label: t("early.unit.mo", { count: earlyOffset }) },
                      { value: 'y', label: t("early.unit.y", { count: earlyOffset }) },
                    ]}
                  />
                  {allDay ? (
                    <label className="early-alert-time">
                      <span>{t("ui.at")}</span>
                      <input type="time" value={earlyTime} onChange={(event) => setEarlyTime(event.target.value)} aria-label={t("ui.early_alert_time")} />
                    </label>
                  ) : null}
                </div>
              ) : null}
            </div>
          </div>

          <div className="detail-control items-start">
            <Repeat2 size={18} className="mt-2 text-violet" />
            <div className="min-w-0 flex-1">
              <span className="detail-control-label">{t("ui.repeat")}</span>
              <div className="repeat-mode mt-2">
                <button type="button" className={repeatUnit === 'none' ? 'is-active' : ''} onClick={() => setRepeatUnit('none')}>{t("ui.don_t_repeat")}</button>
                <button type="button" disabled={!scheduledAt} className={repeatUnit !== 'none' ? 'is-active' : ''} onClick={() => setRepeatUnit(repeatUnit === 'none' ? 'day' : repeatUnit)}>{t("ui.repeat")}</button>
              </div>
              {repeatUnit !== 'none' ? (
                <>
                  <div className="repeat-sentence">
                    <span>{t("ui.every")}</span>
                    <input type="number" min={1} max={999} value={repeatInterval} onChange={(event) => setRepeatInterval(Number(event.target.value) || 1)} aria-label={t("ui.repeat_interval")} />
                    <OneUiSelect
                      ariaLabel={t("ui.repeat_unit")}
                      value={repeatUnit}
                      onChange={(next) => setRepeatUnit(next as RepeatConfig['unit'])}
                      options={(allDay ? ['day', 'week', 'month', 'year'] as const : ['minute', 'hour', 'day', 'week', 'month', 'year'] as const).map((unit) => ({
                        value: unit,
                        label: t(`repeat.unit.${unit}`, { count: repeatInterval }),
                      }))}
                    />
                  </div>
                  {repeatUnit === 'week' ? (
                    <div className="repeat-anchor">
                      <span>{t("repeat.weekdayLabel")}</span>
                      <OneUiSelect
                        ariaLabel={t("ui.repeat_weekday")}
                        value={repeatWeekday}
                        onChange={setRepeatWeekday}
                        options={[t("ui.monday"), t("ui.tuesday"), t("ui.wednesday"), t("ui.thursday"), t("ui.friday"), t("ui.saturday"), t("ui.sunday")].map((label, index) => ({
                          value: ['MO', 'TU', 'WE', 'TH', 'FR', 'SA', 'SU'][index],
                          label,
                        }))}
                      />
                    </div>
                  ) : null}
                  {repeatUnit === 'month' ? (
                    <div className="repeat-anchor">
                      <span>{t("ui.on_day")}</span>
                      <OneUiSelect
                        ariaLabel={t("ui.day_of_month")}
                        value={String(repeatMonthDay)}
                        onChange={(next) => setRepeatMonthDay(Number(next))}
                        options={Array.from({ length: 31 }, (_, day) => ({ value: String(day + 1), label: formatNumber(day + 1) }))}
                      />
                      <span>{t("ui.of_the_month")}</span>
                    </div>
                  ) : null}
                  {repeatUnit === 'year' ? (
                    <div className="repeat-anchor">
                      <span>{t("repeat.monthLabel")}</span>
                      <OneUiSelect
                        ariaLabel={t("ui.repeat_month")}
                        value={String(repeatMonth)}
                        onChange={(next) => setRepeatMonth(Number(next))}
                        options={Array.from({ length: 12 }, (_, month) => ({
                          value: String(month + 1),
                          label: new Intl.DateTimeFormat(getLocale(), { month: 'long' }).format(new Date(2026, month, 1)),
                        }))}
                      />
                      <OneUiSelect
                        ariaLabel={t("ui.day_of_month")}
                        value={String(repeatMonthDay)}
                        onChange={(next) => setRepeatMonthDay(Number(next))}
                        options={Array.from({ length: 31 }, (_, day) => ({ value: String(day + 1), label: formatNumber(day + 1) }))}
                      />
                    </div>
                  ) : null}
                  <div className="mt-2 grid grid-cols-2 gap-2">
                    <OneUiSelect
                      className="detail-select"
                      ariaLabel={t("ui.repeat_duration")}
                      value={repeatEnd}
                      onChange={(next) => setRepeatEnd(next as typeof repeatEnd)}
                      options={[
                        { value: 'forever', label: t("ui.forever") },
                        { value: 'count', label: t("ui.number_of_times") },
                        { value: 'until', label: t("ui.until_date") },
                      ]}
                    />
                    {repeatEnd === 'count' ? <input className="detail-input" type="number" min={1} max={9999} value={repeatCount} onChange={(event) => setRepeatCount(Number(event.target.value) || 1)} aria-label={t("ui.repeat_count")} /> : null}
                    {repeatEnd === 'until' ? <OneUiDatePicker value={repeatUntil} onChange={setRepeatUntil} includeTime={false} /> : null}
                  </div>
                </>
              ) : null}
            </div>
          </div>

          <div className="detail-control items-start">
            <MapPin size={18} className="mt-2 text-place" />
            <div className="min-w-0 flex-1">
              <div className="flex items-center justify-between gap-3">
                <span className="detail-control-label">{t("ui.place")}</span>
                <Switch.Root className="switch-root" checked={locationEnabled} onCheckedChange={setLocationEnabled} aria-label={t("ui.enable_place_trigger")}>
                  <Switch.Thumb className="switch-thumb" />
                </Switch.Root>
              </div>
              {locationEnabled ? (
                <div className="location-editor">
                  <input className="detail-input" value={locationAddress} onChange={(event) => setLocationAddress(event.target.value)} placeholder={t("ui.place_or_address")} aria-label={t("ui.place_address")} />
                  <div className="grid grid-cols-2 gap-2">
                    <input className="detail-input" inputMode="decimal" value={locationLatitude} onChange={(event) => setLocationLatitude(event.target.value)} placeholder={t("ui.latitude")} aria-label={t("ui.latitude")} />
                    <input className="detail-input" inputMode="decimal" value={locationLongitude} onChange={(event) => setLocationLongitude(event.target.value)} placeholder={t("ui.longitude")} aria-label={t("ui.longitude")} />
                  </div>
                  <div className="grid grid-cols-2 gap-2">
                    <OneUiSelect
                      className="detail-select"
                      ariaLabel={t("ui.place_transition")}
                      value={String(locationTransition)}
                      onChange={(next) => setLocationTransition(Number(next))}
                      options={[{ value: '1', label: t("ui.arriving") }, { value: '2', label: t("ui.leaving") }]}
                    />
                    <label className="radius-input"><input type="number" min={50} max={5000} value={locationRadius} onChange={(event) => setLocationRadius(Number(event.target.value) || 200)} aria-label={t("ui.place_radius")} /><span>{t("ui.m")}</span></label>
                  </div>
                  <button
                    type="button"
                    className="text-button w-fit"
                    disabled={!navigator.geolocation}
                    onClick={() => navigator.geolocation?.getCurrentPosition((position) => {
                      setLocationLatitude(position.coords.latitude.toFixed(6));
                      setLocationLongitude(position.coords.longitude.toFixed(6));
                      if (!locationAddress) setLocationAddress(t("ui.current_location"));
                    })}
                  >
                    <MapPin size={14} /> {t("ui.use_current_location")}
                  </button>
                  {!locationValid ? <p className="text-[11px] leading-4 text-danger">{t("ui.latitude_and_longitude_are_required_for_a_samsung_place_trigger")}</p> : null}
                </div>
              ) : null}
            </div>
          </div>

          <div className="detail-control">
            <Volume2 size={18} className="text-important" />
            <span className="detail-control-label">{t("ui.alert")}</span>
            <OneUiSelect
              className="detail-select"
              ariaLabel={t("ui.alert_strength")}
              value={String(alertType)}
              onChange={(next) => setAlertType(Number(next) as 0 | 16 | 17)}
              options={[{ value: '0', label: t("ui.weak") }, { value: '16', label: t("ui.medium") }, { value: '17', label: t("ui.strong") }]}
            />
          </div>

          <div className="detail-control">
            <SelectedCategoryIcon size={18} style={{ color: selectedCategoryVisual.color }} />
            <span className="detail-control-label">{t("ui.list")}</span>
            <OneUiSelect
              className="detail-select"
              ariaLabel={t("ui.reminder_list")}
              value={categoryId}
              onChange={setCategoryId}
              options={categories.map((category) => {
                const visual = categoryVisual(category);
                return {
                  value: category.id,
                  label: category.id === 'LOCAL_SPACE' ? t("ui.my_reminders") : category.name,
                  icon: visual.icon,
                  color: visual.color,
                };
              })}
            />
          </div>
        </div>
      </div>

      <div className="mt-6 flex items-center gap-3">
        <AlertDialog.Root>
          <AlertDialog.Trigger asChild>
            <button type="button" className="danger-icon" aria-label={t("ui.delete_reminder")}><Trash2 size={18} /></button>
          </AlertDialog.Trigger>
          <AlertDialog.Portal>
            <AlertDialog.Overlay className="dialog-overlay" />
            <AlertDialog.Content className="dialog-content max-w-[420px]">
              <AlertDialog.Title className="dialog-title">{t("ui.delete_this_reminder")}</AlertDialog.Title>
              <AlertDialog.Description className="dialog-description">{t("reminder.deleteDescription", { title: reminder.title })}</AlertDialog.Description>
              <div className="mt-7 flex justify-end gap-3">
                <AlertDialog.Cancel asChild><button className="secondary-button">{t("ui.cancel")}</button></AlertDialog.Cancel>
                <AlertDialog.Action asChild><button className="delete-button" onClick={() => onDelete(reminder.id)}>{t("ui.delete")}</button></AlertDialog.Action>
              </div>
            </AlertDialog.Content>
          </AlertDialog.Portal>
        </AlertDialog.Root>
        <button type="button" className="secondary-button px-3" onClick={() => void shareReminder()} aria-label={t("ui.share_reminder")}><Share2 size={17} /></button>
        <button
          type="button"
          className="primary-button ml-auto min-w-28"
          disabled={!dirty || saving || !title.trim() || !locationValid}
          onClick={() => onSave({
            id: reminder.id,
            title: title.trim(),
            text,
            completed,
            favorite,
            reminderAt: scheduledAt
              ? allDay
                ? new Date(`${scheduledAt.slice(0, 10)}T00:00:00.000Z`).toISOString()
                : new Date(scheduledAt).toISOString()
              : null,
            allDay,
            repeat: repeatValue(),
            alertType,
            earlyAlert: earlyAlertValue(),
            categoryId,
            checklist: normalizedChecklist,
            ...(locationDirty ? { location: locationValue } : {}),
          })}
        >
          {saving ? <LoaderCircle size={17} className="animate-spin" /> : null}
          {saving ? t("ui.saving") : t("ui.save")}
        </button>
      </div>
    </aside>
  );
}

function CategoryManager({ trigger, categories, counts, busy, onCreate, onUpdate, onDelete }: {
  trigger: React.ReactElement;
  categories: ReminderCategory[];
  counts: Record<string, number>;
  busy: boolean;
  onCreate: (values: { name: string; color: number; iconIndex: number }) => Promise<void>;
  onUpdate: (values: { id: string; name: string; color: number; iconIndex: number }) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
}) {
  useI18n();
  const [open, setOpen] = useState(false);
  const [newName, setNewName] = useState('');
  const [newColor, setNewColor] = useState(6);
  const [newIconIndex, setNewIconIndex] = useState(11);
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [drafts, setDrafts] = useState<Record<string, { name: string; color: number; iconIndex: number }>>({});

  useEffect(() => {
    setDrafts(Object.fromEntries(categories.map((category) => [category.id, {
      name: category.name,
      color: category.color,
      iconIndex: category.iconIndex,
    }])));
  }, [categories]);

  async function createCategory() {
    const name = newName.trim();
    if (!name) return;
    await onCreate({ name, color: newColor, iconIndex: newIconIndex })
      .then(() => setNewName(''))
      .catch(() => undefined);
  }

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger asChild>{trigger}</Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content className="dialog-content category-manager-dialog">
          <div className="flex items-start justify-between gap-5">
            <div>
              <Dialog.Title className="dialog-title">{t("ui.reminder_lists")}</Dialog.Title>
              <Dialog.Description className="dialog-description">{t("ui.create_and_edit_the_lists_synced_with_samsung_reminder")}</Dialog.Description>
            </div>
            <Dialog.Close asChild><IconButton label={t("ui.close")}><X size={19} /></IconButton></Dialog.Close>
          </div>

          <section className="category-create-panel">
            <div className="category-create-copy">
              <CategoryIconPicker value={newIconIndex} color={newColor} label={t("ui.choose_icon_for_new_list")} onChange={setNewIconIndex} />
              <div>
                <strong>{t("ui.new_list")}</strong>
                <span>{t("ui.choose_a_name_color_and_icon")}</span>
              </div>
            </div>
            <div className="category-create-fields">
              <input value={newName} onChange={(event) => setNewName(event.target.value)} placeholder={t("ui.list_name")} maxLength={80} onKeyDown={(event) => { if (event.key === 'Enter') void createCategory(); }} />
              <div className="category-swatches" aria-label={t("ui.new_list_color")}>
                {categoryColorOrder.map((index) => <button type="button" key={index} aria-label={categoryColorName(index)} aria-pressed={newColor === index} style={{ '--swatch': categoryColor(index) } as React.CSSProperties} onClick={() => setNewColor(index)} />)}
              </div>
              <button type="button" className="primary-button" disabled={!newName.trim() || busy} onClick={() => void createCategory()}>{busy ? <LoaderCircle size={16} className="animate-spin" /> : <Plus size={16} />} {t("ui.create")}</button>
            </div>
          </section>

          <div className="category-editor-list">
            {categories.map((category) => {
              const isDefault = category.id === 'LOCAL_SPACE';
              const draft = drafts[category.id] || { name: category.name, color: category.color, iconIndex: category.iconIndex };
              const changed = draft.name.trim() !== category.name
                || draft.color !== category.color
                || draft.iconIndex !== category.iconIndex;
              const count = counts[`category:${category.id}`] || 0;
              return (
                <div className={`category-editor-row ${isDefault ? 'is-default' : ''}`} key={category.id}>
                  <CategoryIconPicker
                    value={draft.iconIndex}
                    color={draft.color}
                    label={t("category.chooseIcon", { name: isDefault ? t("ui.my_reminders") : category.name })}
                    disabled={isDefault}
                    onChange={(iconIndex) => setDrafts((current) => ({ ...current, [category.id]: { ...draft, iconIndex } }))}
                  />
                  <div className="min-w-0 flex-1">
                    {isDefault ? (
                      <div className="category-locked-name"><strong>{t("ui.my_reminders")}</strong><span><LockKeyhole size={11} /> {t("ui.default")}</span></div>
                    ) : (
                      <input className="category-name-input" value={draft.name} maxLength={80} aria-label={t("category.name", { name: category.name })} onChange={(event) => setDrafts((current) => ({ ...current, [category.id]: { ...draft, name: event.target.value } }))} />
                    )}
                    <span className="category-count">{t("reminder.count", { count })}</span>
                    {!isDefault ? (
                      <div className="category-swatches compact" aria-label={t("category.color", { name: category.name })}>
                        {categoryColorOrder.map((index) => <button type="button" key={index} aria-label={categoryColorName(index)} aria-pressed={draft.color === index} style={{ '--swatch': categoryColor(index) } as React.CSSProperties} onClick={() => setDrafts((current) => ({ ...current, [category.id]: { ...draft, color: index } }))} />)}
                      </div>
                    ) : null}
                  </div>
                  {!isDefault ? (
                    <div className="category-row-actions">
                      {deleteId === category.id ? (
                        <div className="category-delete-confirm">
                          <span>{t("ui.move_its_reminders_to_my_reminders")}</span>
                          <button type="button" onClick={() => setDeleteId(null)}>{t("ui.cancel")}</button>
                          <button type="button" className="is-danger" disabled={busy} onClick={() => void onDelete(category.id).then(() => setDeleteId(null)).catch(() => undefined)}>{t("ui.delete")}</button>
                        </div>
                      ) : (
                        <>
                          <button type="button" className="category-save-button" aria-label={t("category.save", { name: category.name })} disabled={!changed || !draft.name.trim() || busy} onClick={() => void onUpdate({ id: category.id, name: draft.name.trim(), color: draft.color, iconIndex: draft.iconIndex }).catch(() => undefined)}><Save size={16} /></button>
                          <button type="button" className="category-remove-button" aria-label={t("category.delete", { name: category.name })} disabled={busy} onClick={() => setDeleteId(category.id)}><Trash2 size={16} /></button>
                        </>
                      )}
                    </div>
                  ) : null}
                </div>
              );
            })}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function SettingsDialog({ endpoint, setEndpoint, theme, setTheme, connected, onDisconnect, onWipe }: SettingsDialogProps) {
  const { preference, setLanguage } = useI18n();
  const [draftEndpoint, setDraftEndpoint] = useState(endpoint);
  const [disconnecting, setDisconnecting] = useState(false);
  const [disconnectError, setDisconnectError] = useState<string | null>(null);
  const [wiping, setWiping] = useState(false);
  const [wipeError, setWipeError] = useState<string | null>(null);
  const mcpConfig = `[mcp_servers.samsung-reminders]\ncommand = "C:/path/to/samsung-reminder-mcp.exe"\nenv = { CDP_ENDPOINT = "${endpoint}" }`;

  return (
    <Dialog.Portal>
      <Dialog.Overlay className="dialog-overlay" />
      <Dialog.Content className="dialog-content w-[min(540px,calc(100vw-32px))]">
        <div className="flex items-center justify-between">
          <Dialog.Title className="dialog-title">{t("ui.settings")}</Dialog.Title>
          <Dialog.Close asChild><IconButton label={t("ui.close")}><X size={19} /></IconButton></Dialog.Close>
        </div>
        <Dialog.Description className="dialog-description">{t("ui.connection_appearance_and_mcp_access")}</Dialog.Description>

        <div className="mt-7 space-y-6">
          <section><label className="settings-label">{t("language.label")}</label><OneUiSelect ariaLabel={t("language.label")} value={preference} onChange={(next) => setLanguage(next as 'system' | 'en' | 'ru')} options={[{ value: 'system', label: t("language.system") }, { value: 'ru', label: 'Русский' }, { value: 'en', label: 'English' }]} /></section>
          <section>
            <div className="mb-3 flex items-center justify-between">
              <label htmlFor="endpoint" className="settings-label">{t("ui.samsung_browser_bridge")}</label>
              <span className={`status-chip ${connected ? 'is-online' : ''}`}>
                <span className="size-1.5 rounded-full bg-current" />
                {connected ? t("ui.connected") : t("ui.ready_on_demand")}
              </span>
            </div>
            <input id="endpoint" className="settings-input" value={draftEndpoint} onChange={(event) => setDraftEndpoint(event.target.value)} />
            <p className="mt-2 text-xs leading-5 text-muted">{t("ui.the_first_sync_signs_in_through_your_existing_samsung_browser_profile_securely_remembers_the_session_then_closes_the_browser_completely_it_signs_in_again_only_when_samsung_requires_it")}</p>
            <button className="text-button mt-3" type="button" onClick={() => {
              localStorage.setItem('reminder-cdp-endpoint', draftEndpoint);
              setEndpoint(draftEndpoint);
            }}>{t("ui.save_endpoint")}</button>
          </section>

          <Separator.Root className="h-px bg-line" />
          <section className="flex items-center justify-between">
            <div>
              <p className="settings-label">{t("ui.dark_appearance")}</p>
              <p className="mt-1 text-xs text-muted">{t("ui.match_samsung_reminder_s_deep_black_theme")}</p>
            </div>
            <Switch.Root
              className="switch-root"
              checked={theme === 'dark'}
              onCheckedChange={(checked) => setTheme(checked ? 'dark' : 'light')}
            >
              <Switch.Thumb className="switch-thumb" />
            </Switch.Root>
          </section>

          <Separator.Root className="h-px bg-line" />
          <section>
            <p className="settings-label">{t("ui.samsung_account_on_this_pc")}</p>
            <p className="mt-1 text-xs leading-5 text-muted">{t("ui.remove_the_cached_samsung_cloud_credential_from_windows_credential_manager_and_return_to_the_connection_disclosure")}</p>
            {disconnectError ? <p className="mt-2 text-xs text-danger" role="alert">{t(disconnectError)}</p> : null}
            <button
              className="text-button mt-3"
              type="button"
              disabled={disconnecting}
              onClick={() => {
                setDisconnecting(true);
                setDisconnectError(null);
                void onDisconnect().catch((error) => {
                  setDisconnectError("error.operation");
                  setDisconnecting(false);
                });
              }}
            >
              {disconnecting ? t("ui.disconnecting") : t("ui.disconnect_samsung_account")}
            </button>
          </section>

          <Separator.Root className="h-px bg-line" />
          <section>
            <p className="settings-label">{t("ui.mcp_server")}</p>
            <p className="mt-1 text-xs leading-5 text-muted">{t("ui.build_the_sibling_rust_binary_with")} <code>pnpm build:mcp</code>{t("ui.then_add_it_to_codex")}</p>
            <div className="mt-3 rounded-[18px] bg-code p-4 font-mono text-[11px] leading-5 text-code-ink">
              <pre className="whitespace-pre-wrap">{mcpConfig}</pre>
            </div>
            <button className="text-button mt-3" type="button" onClick={() => navigator.clipboard.writeText(mcpConfig)}>{t("ui.copy_configuration")}</button>
          </section>

          <Separator.Root className="h-px bg-line" />
          <section>
            <p className="settings-label">{t("ui.erase_all_app_data")}</p>
            <p className="mt-1 text-xs leading-5 text-muted">{t("ui.remove_everything_this_app_stores_on_this_pc_and_start_over_from_the_connection_disclosure_reminders_and_lists_in_samsung_cloud_are_not_deleted")}</p>
            {wipeError ? <p className="mt-2 text-xs text-danger" role="alert">{t(wipeError)}</p> : null}
            <AlertDialog.Root>
              <AlertDialog.Trigger asChild>
                <button className="wipe-button mt-3" type="button" disabled={wiping}>
                  <Trash2 size={15} /> {wiping ? t("ui.erasing") : t("ui.erase_app_data")}
                </button>
              </AlertDialog.Trigger>
              <AlertDialog.Portal>
                <AlertDialog.Overlay className="dialog-overlay" />
                <AlertDialog.Content className="dialog-content max-w-[440px]">
                  <AlertDialog.Title className="dialog-title">{t("ui.erase_all_app_data")}</AlertDialog.Title>
                  <AlertDialog.Description className="dialog-description">{t("ui.this_resets_the_app_to_how_it_was_after_installation_and_removes")}</AlertDialog.Description>
                  <ul className="mt-4 list-disc space-y-1.5 pl-5 text-[13px] leading-5 text-muted marker:text-muted/60">
                    <li>{t("ui.the_cached_samsung_cloud_credential_in_windows_credential_manager")}</li>
                    <li>{t("ui.your_consent_choice_for_cloud_access")}</li>
                    <li>{t("ui.local_settings_including_the_bridge_endpoint_and_appearance")}</li>
                  </ul>
                  <p className="mt-4 text-[13px] leading-5 text-muted">{t("ui.your_reminders_stay_safe_in_samsung_cloud_the_app_restarts_at_the_connection_disclosure")}</p>
                  <div className="mt-7 flex justify-end gap-3">
                    <AlertDialog.Cancel asChild><button className="secondary-button">{t("ui.cancel")}</button></AlertDialog.Cancel>
                    <AlertDialog.Action asChild>
                      <button
                        className="delete-button"
                        onClick={() => {
                          setWiping(true);
                          setWipeError(null);
                          void onWipe().catch((error) => {
                            setWipeError("error.operation");
                            setWiping(false);
                          });
                        }}
                      >
                        {t("ui.erase_and_restart")}
                      </button>
                    </AlertDialog.Action>
                  </div>
                </AlertDialog.Content>
              </AlertDialog.Portal>
            </AlertDialog.Root>
          </section>
        </div>
      </Dialog.Content>
    </Dialog.Portal>
  );
}

function ReminderApp() {
  const { language } = useI18n();
  const categoryDefinitions = getCategoryDefinitions();
  const queryClient = useQueryClient();
  const [category, setCategory] = useState<CategoryId>('all');
  const [search, setSearch] = useState('');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [quickTitle, setQuickTitle] = useState('');
  const [endpoint, setEndpoint] = useState(() => localStorage.getItem('reminder-cdp-endpoint') || 'http://127.0.0.1:9226');
  const [theme, setTheme] = useState<Theme>(() => (localStorage.getItem('reminder-theme') as Theme) || 'dark');
  const [notice, setNotice] = useState<Notice | null>(null);
  const [pendingWrites, setPendingWrites] = useState(0);
  const [manualSyncing, setManualSyncing] = useState(false);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem('reminder-theme', theme);
  }, [theme]);

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'n') {
        event.preventDefault();
        document.getElementById('quick-add')?.focus();
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);

  const probeQuery = useQuery({
    queryKey: ['reminder-status', endpoint],
    queryFn: () => reminderOperation<BridgeProbe>('probe', {}, endpoint),
    retry: false,
    refetchOnReconnect: false,
  });
  const collections = useMemo(
    () => createReminderCollections(queryClient, endpoint, false),
    [queryClient, endpoint],
  );
  const remindersLive = useLiveQuery(() => collections.reminders, [collections.reminders]);
  const categoriesLive = useLiveQuery(() => collections.categories, [collections.categories]);

  // A failed connectivity check must not replace the collections and discard
  // their last successful rows. Explicit refetch also works while disabled.
  useEffect(() => {
    if (!probeQuery.isSuccess) return;
    void Promise.all([
      collections.reminders.utils.refetch(),
      collections.categories.utils.refetch(),
    ]).catch(() => undefined);
  }, [collections, probeQuery.isSuccess]);

  const reminders = remindersLive.data || [];
  const categories = useMemo(() => {
    const received = categoriesLive.data || [];
    const local = received.find((item) => item.id === 'LOCAL_SPACE') || { id: 'LOCAL_SPACE', name: t("ui.my_reminders"), color: 0, iconIndex: 0, order: -1, extensionInfo: null };
    return [local, ...received.filter((item) => item.id !== 'LOCAL_SPACE').sort((a, b) => a.order - b.order || a.name.localeCompare(b.name))];
  }, [categoriesLive.data, language]);
  const customCategoryDefinitions = useMemo(() => categories.map((item) => {
    const visual = categoryVisual(item);
    return { id: `category:${item.id}`, label: item.id === 'LOCAL_SPACE' ? t("ui.my_reminders") : item.name, ...visual };
  }), [categories, language]);
  const counts = useMemo(() => Object.fromEntries(
    [...categoryDefinitions, ...customCategoryDefinitions]
      .map((definition) => [definition.id, reminders.filter((item) => isInCategory(item, definition.id)).length]),
  ) as Record<string, number>, [reminders, customCategoryDefinitions]);
  const filtered = useMemo(() => reminders
    .filter((item) => isInCategory(item, category))
    .filter((item) => !search || `${item.title} ${item.text}`.toLocaleLowerCase().includes(search.toLocaleLowerCase()))
    .sort((a, b) => {
      const completion = Number(a.completed) - Number(b.completed);
      if (completion) return completion;
      const firstDate = a.reminderAt || a.startsAt;
      const secondDate = b.reminderAt || b.startsAt;
      if (firstDate && secondDate) return firstDate.localeCompare(secondDate);
      if (firstDate) return -1;
      if (secondDate) return 1;
      return Number(b.favorite) - Number(a.favorite)
        || String(b.createdAt).localeCompare(String(a.createdAt));
    }), [reminders, category, search]);
  const grouped = useMemo(() => {
    const groups = new Map<string, Reminder[]>();
    for (const reminder of filtered) {
      const label = reminderGroupLabel(reminder);
      groups.set(label, [...(groups.get(label) || []), reminder]);
    }
    return [...groups.entries()];
  }, [filtered, language]);
  const selected = reminders.find((item) => item.id === selectedId) || null;

  useEffect(() => {
    if (filtered.length && !filtered.some((item) => item.id === selectedId)) {
      setSelectedId(filtered[0].id);
    }
    if (!filtered.length && selectedId) {
      setSelectedId(null);
      setDetailsOpen(false);
    }
  }, [filtered, selectedId]);

  const refresh = async () => {
    setManualSyncing(true);
    try {
      const wasConnected = probeQuery.isSuccess;
      const probeResult = await probeQuery.refetch();
      // The effect owns the initial/recovery load; avoid launching it twice.
      if (!probeResult.isSuccess || !wasConnected) return;
      await Promise.all([
        collections.reminders.utils.refetch(),
        collections.categories.utils.refetch(),
      ]);
    } finally {
      setManualSyncing(false);
    }
  };

  async function persistTransaction(transaction: PersistableTransaction, message: string) {
    setPendingWrites((count) => count + 1);
    try {
      await transaction.isPersisted.promise;
      setNotice({ message, kind: 'success' });
    } catch (error) {
      setNotice({ message: "error.operation", kind: 'error' });
      throw error;
    } finally {
      setPendingWrites((count) => Math.max(0, count - 1));
    }
  }

  function observeTransaction(transaction: PersistableTransaction, message: string) {
    void persistTransaction(transaction, message).catch(() => undefined);
  }

  function createReminder(title: string) {
    const id = crypto.randomUUID();
    const reminder = makeOptimisticReminder({
      title,
      text: '',
      categoryId: category.startsWith('category:') ? category.slice('category:'.length) : 'LOCAL_SPACE',
    }, id);
    const transaction = collections.reminders.insert(reminder);
    setSelectedId(id);
    setQuickTitle('');
    observeTransaction(transaction, "ui.reminder_added");
  }

  function updateReminder(values: Record<string, unknown>, message = "ui.changes_saved") {
    const id = String(values.id || '');
    const current = reminders.find((reminder) => reminder.id === id);
    if (!current) return;
    const modified = applyReminderPatch(current, values);
    const transaction = collections.reminders.update(id, (draft) => Object.assign(draft, modified));
    observeTransaction(transaction, message);
  }

  function deleteReminder(id: string) {
    const transaction = collections.reminders.delete(id);
    setSelectedId(null);
    setDetailsOpen(false);
    observeTransaction(transaction, "ui.reminder_deleted");
  }

  async function disconnectAccount() {
    await clearReminderCredential();
    localStorage.removeItem(consentStorageKey);
    window.location.reload();
  }

  async function wipeAppData() {
    await clearReminderCredential();
    localStorage.clear();
    window.location.reload();
  }

  useEffect(() => {
    if (!notice) return;
    const timeout = window.setTimeout(() => setNotice(null), notice.kind === 'error' ? 5000 : 2200);
    return () => window.clearTimeout(timeout);
  }, [notice]);

  function submitQuickAdd(event: FormEvent) {
    event.preventDefault();
    const title = quickTitle.trim();
    if (title) createReminder(title);
  }

  const categoryLabel = category === 'all'
    ? t("ui.my_reminders")
    : [...categoryDefinitions, ...customCategoryDefinitions].find((item) => item.id === category)?.label || t("ui.reminders");
  const syncError = probeQuery.error
    || collections.reminders.utils.lastError
    || collections.categories.utils.lastError;
  const connected = probeQuery.isSuccess
    && remindersLive.isReady
    && !collections.reminders.utils.isError;
  const loading = !syncError && (probeQuery.isPending || (probeQuery.isSuccess
    && (remindersLive.isLoading || collections.reminders.utils.isLoading)));
  const syncing = manualSyncing || loading;
  const syncIssue = connectionIssue(syncError);
  const SyncIssueIcon = syncIssue.icon;

  return (
    <div className="min-h-screen bg-canvas text-ink">
      <header className="app-header">
        <div className="flex min-w-0 items-center gap-3">
          <div className="app-mark"><Check size={22} strokeWidth={3.2} /></div>
          <h1 className="truncate text-[25px] font-bold tracking-[-0.04em] text-brand">{t("ui.reminder")}</h1>
          {isPreview ? <span className="preview-chip">{t("ui.preview")}</span> : null}
        </div>
        <div className="ml-auto flex items-center gap-2">
          <label className="search-box">
            <Search size={17} />
            <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder={t("ui.search_reminders")} aria-label={t("ui.search_reminders")} />
            {search ? <button type="button" onClick={() => setSearch('')} aria-label={t("ui.clear_search")}><X size={15} /></button> : null}
          </label>
          <IconButton label={t("ui.sync_now")} onClick={() => void refresh()} className={syncing ? '[&>svg]:animate-spin' : ''}>
            <RefreshCw size={18} />
          </IconButton>
          <DropdownMenu.Root>
            <DropdownMenu.Trigger className="icon-button" aria-label={t("ui.more_options")}><EllipsisVertical size={20} /></DropdownMenu.Trigger>
            <DropdownMenu.Portal>
              <DropdownMenu.Content align="end" sideOffset={8} className="menu-content">
                <DropdownMenu.Label className="account-menu-summary">
                  <span className="account-menu-avatar"><CircleUserRound size={21} /></span>
                  <span className="account-menu-copy">
                    <strong>{t("ui.samsung_account")}</strong>
                    <span>{probeQuery.data?.accountEmail || (connected ? t("ui.connected") : t("ui.not_connected"))}</span>
                  </span>
                  <span className={`account-status-dot ${connected ? 'is-online' : ''}`} aria-label={connected ? t("ui.connected") : t("ui.offline")} />
                </DropdownMenu.Label>
                <DropdownMenu.Separator className="menu-separator" />
                <CategoryManager
                  trigger={
                    <DropdownMenu.Item className="menu-item" onSelect={(event) => event.preventDefault()}>
                      <FolderCog size={16} /> {t("ui.manage_categories")}
                    </DropdownMenu.Item>
                  }
                  categories={categories}
                  counts={counts}
                  busy={pendingWrites > 0}
                  onCreate={async (values) => {
                    const id = crypto.randomUUID();
                    const transaction = collections.categories.insert({
                      id,
                      ...values,
                      order: categories.length,
                      extensionInfo: null,
                    });
                    await persistTransaction(transaction, "ui.list_created");
                  }}
                  onUpdate={async (values) => {
                    const transaction = collections.categories.update(values.id, (draft) => Object.assign(draft, values));
                    await persistTransaction(transaction, "ui.list_updated");
                  }}
                  onDelete={async (id) => {
                    const transaction = collections.categories.delete(id);
                    await persistTransaction(transaction, "ui.list_deleted");
                    if (category === `category:${id}`) setCategory('all');
                  }}
                />
                <DropdownMenu.Item className="menu-item" onSelect={() => setTheme(theme === 'dark' ? 'light' : 'dark')}>
                  {theme === 'dark' ? <Sun size={16} /> : <Moon size={16} />} {theme === 'dark' ? t("ui.light_appearance") : t("ui.dark_appearance")}
                </DropdownMenu.Item>
                <Dialog.Root>
                  <Dialog.Trigger asChild>
                    <DropdownMenu.Item className="menu-item" onSelect={(event) => event.preventDefault()}><Settings2 size={16} /> {t("ui.settings")}</DropdownMenu.Item>
                  </Dialog.Trigger>
                  <SettingsDialog
                    endpoint={endpoint}
                    setEndpoint={setEndpoint}
                    theme={theme}
                    setTheme={setTheme}
                    connected={connected}
                    onDisconnect={disconnectAccount}
                    onWipe={wipeAppData}
                  />
                </Dialog.Root>
              </DropdownMenu.Content>
            </DropdownMenu.Portal>
          </DropdownMenu.Root>
        </div>
      </header>

      <div className="app-body">
        <main className="main-pane">
          <ScrollArea.Root className="min-h-0 flex-1 overflow-hidden">
            <ScrollArea.Viewport className="size-full">
              <div className="content-stack">
                <section aria-label={t("ui.reminder_categories")}>
                  <div className="mb-4 flex items-baseline justify-between">
                    <button type="button" onClick={() => setCategory('all')} className="section-title">{t("ui.categories")}</button>
                    <span className="text-xs text-muted">{t("reminder.open", { count: reminders.filter((item) => !item.completed).length })}</span>
                  </div>
                  <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-3">
                    {categoryDefinitions.map((definition) => (
                      <CategoryCard
                        key={definition.id}
                        definition={definition}
                        count={counts[definition.id]}
                        selected={category === definition.id}
                        onSelect={() => setCategory((current) => current === definition.id ? 'all' : definition.id)}
                      />
                    ))}
                  </div>
                  {customCategoryDefinitions.length ? (
                    <>
                      <div className="my-4 flex items-center gap-3">
                        <Separator.Root className="h-px flex-1 bg-line" />
                        <ChevronRight size={16} className="rotate-90 text-muted" />
                        <Separator.Root className="h-px flex-1 bg-line" />
                      </div>
                      <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-3">
                        {customCategoryDefinitions.map((definition) => (
                          <CategoryCard
                            key={definition.id}
                            definition={definition}
                            count={counts[definition.id]}
                            selected={category === definition.id}
                            onSelect={() => setCategory((current) => current === definition.id ? 'all' : definition.id)}
                          />
                        ))}
                      </div>
                    </>
                  ) : null}
                </section>

                <section className="pb-32">
                  <div className="mb-4 flex items-end justify-between">
                    <div>
                      <h2 className="section-title">{categoryLabel}</h2>
                      <p className="mt-1 text-xs text-muted">{t("reminder.count", { count: filtered.length })}</p>
                    </div>
                  </div>

                  {!connected && !isPreview && !loading ? (
                    <div className="connection-card">
                      <div className="grid size-12 place-items-center rounded-[18px] bg-violet-soft text-violet"><SyncIssueIcon size={23} /></div>
                      <div className="min-w-0 flex-1">
                        <h3 className="font-semibold text-ink">{syncIssue.title}</h3>
                        <p className="mt-1 text-sm leading-5 text-muted">{syncIssue.detail}</p>
                      </div>
                      <button className="secondary-button" disabled={manualSyncing} onClick={() => void refresh()}>
                        {manualSyncing ? t("ui.checking") : syncIssue.action}
                      </button>
                    </div>
                  ) : loading ? (
                    <div className="grid min-h-40 place-items-center text-muted"><LoaderCircle size={24} className="animate-spin" /></div>
                  ) : filtered.length ? (
                    <div className="space-y-5">
                      {grouped.map(([label, group]) => (
                        <div key={label}>
                          <h3 className="mb-2 px-2 text-[13px] font-semibold text-muted">{label}</h3>
                          <div className="reminder-list">
                            {group.map((reminder) => (
                              <ReminderRow
                                key={reminder.id}
                                reminder={reminder}
                                selected={reminder.id === selectedId}
                                onSelect={() => { setSelectedId(reminder.id); setDetailsOpen(true); }}
                                onToggle={() => updateReminder({ id: reminder.id, completed: !reminder.completed }, reminder.completed ? "ui.reminder_reopened" : "ui.reminder_completed")}
                              />
                            ))}
                          </div>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <div className="empty-state">
                      <Circle size={30} strokeWidth={1.4} />
                      <p className="mt-3 font-medium text-ink">{t("ui.nothing_here")}</p>
                      <p className="mt-1 text-sm text-muted">{t("ui.add_a_reminder_or_choose_another_category")}</p>
                    </div>
                  )}
                </section>
              </div>
            </ScrollArea.Viewport>
            <ScrollArea.Scrollbar orientation="vertical" className="scrollbar"><ScrollArea.Thumb className="scrollbar-thumb" /></ScrollArea.Scrollbar>
          </ScrollArea.Root>

          <div className="quick-add-wrap">
            <form className="quick-add" onSubmit={submitQuickAdd}>
              <span className="grid size-9 shrink-0 place-items-center rounded-full text-muted"><Plus size={24} strokeWidth={1.8} /></span>
              <input id="quick-add" value={quickTitle} onChange={(event) => setQuickTitle(event.target.value)} placeholder={t("ui.add_reminder")} autoComplete="off" />
              {pendingWrites > 0 ? <LoaderCircle size={20} className="animate-spin text-brand" /> : quickTitle.trim() ? (
                <button type="submit" className="quick-submit" aria-label={t("ui.add_reminder")}><Check size={19} strokeWidth={3} /></button>
              ) : (
                <button type="button" className="grid size-9 place-items-center text-muted" aria-label={t("ui.voice_input_unavailable")}><Mic size={21} /></button>
              )}
            </form>
          </div>
        </main>

        {detailsOpen && selected ? (
          <button
            type="button"
            className="drawer-scrim lg:hidden"
            aria-label={t("ui.close_details")}
            onClick={() => setDetailsOpen(false)}
          />
        ) : null}
        <Inspector
          reminder={selected}
          categories={categories}
          saving={pendingWrites > 0}
          open={detailsOpen}
          onClose={() => setDetailsOpen(false)}
          onSave={(values) => updateReminder(values)}
          onDelete={deleteReminder}
        />
      </div>

      {notice ? (
        <div className={`toast ${notice.kind === 'error' ? 'is-error' : ''}`} role={notice.kind === 'error' ? 'alert' : 'status'}>
          {notice.kind === 'error' ? <TriangleAlert size={16} /> : <Check size={15} strokeWidth={3} />}
          {t(notice.message)}
        </div>
      ) : null}
    </div>
  );
}

function ConsentGate({ state, onContinue, onCancel, onReview }: {
  state: Exclude<ConsentState, 'accepted'>;
  onContinue: () => string | null;
  onCancel: () => void;
  onReview: () => void;
}) {
  useI18n();
  const [acknowledged, setAcknowledged] = useState(false);
  const [storageError, setStorageError] = useState<string | null>(null);

  async function exitApplication() {
    if (isPreview) {
      window.close();
      return;
    }

    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().close();
    } catch {
      window.close();
    }
  }

  function continueToApp() {
    const error = onContinue();
    setStorageError(error);
  }

  if (state === 'declined') {
    return (
      <main className="consent-screen consent-screen-disabled">
        <div className="consent-orbit consent-orbit-one" aria-hidden="true" />
        <div className="consent-orbit consent-orbit-two" aria-hidden="true" />
        <section className="consent-disabled-card" aria-labelledby="cloud-disabled-title">
          <div className="consent-disabled-icon"><LockKeyhole size={28} strokeWidth={2} /></div>
          <p className="consent-kicker">{t("ui.connection_cancelled")}</p>
          <h1 id="cloud-disabled-title">{t("ui.cloud_access_stays_off")}</h1>
          <p className="consent-disabled-copy">
            {t("ui.no_reminder_collections_categories_account_probe_browser_bootstrap_or_samsung_cloud_request_has_been_started")}
          </p>
          <div className="consent-disabled-actions">
            <button type="button" className="consent-button consent-button-secondary" onClick={onReview}>{t("ui.review_disclosure")}</button>
            <button type="button" className="consent-button consent-button-quiet" onClick={() => void exitApplication()}>{t("ui.exit")}</button>
          </div>
        </section>
      </main>
    );
  }

  return (
    <main className="consent-screen">
      <div className="consent-orbit consent-orbit-one" aria-hidden="true" />
      <div className="consent-orbit consent-orbit-two" aria-hidden="true" />
      <div className="consent-shell">
        <header className="consent-brand-row">
          <div className="consent-brand">
            <span className="consent-brand-mark"><Check size={20} strokeWidth={3.2} /></span>
            <span>{t("ui.reminder")}</span>
          </div>
        </header>

        <section className="consent-panel" aria-labelledby="consent-title" aria-describedby="consent-summary">
          <div className="consent-intro">
            <p className="consent-kicker">{t("ui.before_cloud_access")}</p>
            <h1 id="consent-title">{t("ui.know_what_connects")}</h1>
            <p id="consent-summary">
              {t("ui.this_is_an_unofficial_samsung_reminder_client_it_relies_on_private_undocumented_samsung_behavior_and_is_not_supported_by_samsung")}
            </p>

            <div className="consent-path" aria-label={t("ui.how_the_connection_works")}>
              <article className="consent-path-step">
                <span className="consent-step-number">01</span>
                <span className="consent-step-icon"><LogIn size={19} /></span>
                <div>
                  <h2>{t("ui.browser_bootstrap")}</h2>
                  <p>{t("ui.starts_samsung_browser_hidden_on_a_localhost_cdp_port_and_opens_its_already_signed_in_calendar_extension")}</p>
                </div>
              </article>
              <article className="consent-path-step">
                <span className="consent-step-number">02</span>
                <span className="consent-step-icon"><LockKeyhole size={19} /></span>
                <div>
                  <h2>{t("ui.temporary_credential")}</h2>
                  <p>{t("ui.acquires_a_temporary_samsung_token_and_stores_it_in_windows_credential_manager_not_in_app_files")}</p>
                </div>
              </article>
              <article className="consent-path-step">
                <span className="consent-step-number">03</span>
                <span className="consent-step-icon"><CalendarDays size={19} /></span>
                <div>
                  <h2>{t("ui.direct_cloud_access")}</h2>
                  <p>{t("ui.reads_and_writes_reminders_and_categories_directly_in_samsung_cloud_then_closes_the_browser_after_bootstrap")}</p>
                </div>
              </article>
            </div>
          </div>

          <aside className="consent-decision" aria-label={t("ui.risks_and_privacy")}>
            <div className="consent-risk">
              <span><TriangleAlert size={19} /></span>
              <div>
                <h2>{t("ui.unsupported_and_potentially_risky")}</h2>
                <p>{t("ui.samsung_can_change_or_block_this_behavior_at_any_time_breakage_or_bugs_could_affect_cloud_reminder_data_or_put_your_samsung_account_at_risk")}</p>
              </div>
            </div>

            <div className="consent-privacy">
              <span className="consent-privacy-icon"><CircleUserRound size={20} /></span>
              <div>
                <p className="consent-privacy-label">{t("ui.publisher_receives_no_data")}</p>
                <p>{t("ui.your_reminders_account_details_token_and_usage_stay_between_this_pc_and_samsung_cloud_there_is_no_publisher_relay_or_telemetry")}</p>
              </div>
            </div>

            {isPreview ? (
              <p className="consent-preview-note"><span>{t("ui.preview")}</span> {t("ui.this_browser_demo_uses_local_sample_data_only_and_does_not_contact_samsung")}</p>
            ) : null}

            <label className="consent-acknowledgement">
              <input
                type="checkbox"
                checked={acknowledged}
                onChange={(event) => {
                  setAcknowledged(event.target.checked);
                  setStorageError(null);
                }}
              />
              <span className="consent-checkbox" aria-hidden="true"><Check size={14} strokeWidth={3} /></span>
              <span>{t("ui.i_understand_this_is_unofficial_and_authorize_direct_access_to_my_samsung_cloud_reminders_from_this_pc")}</span>
            </label>

            {storageError ? <p className="consent-storage-error" role="alert">{t(storageError)}</p> : null}

            <div className="consent-actions">
              <button type="button" className="consent-button consent-button-quiet" onClick={onCancel}>{t("ui.cancel")}</button>
              <button
                type="button"
                className="consent-button consent-button-primary"
                disabled={!acknowledged}
                onClick={continueToApp}
              >
                {t("ui.continue")} <ChevronRight size={17} />
              </button>
            </div>
            <p className="consent-footnote">{t("ui.your_choice_is_stored_locally_on_this_device")}</p>
          </aside>
        </section>
      </div>
    </main>
  );
}

export default function App() {
  useI18n();
  const [consent, setConsent] = useState<ConsentState>(() => hasStoredConsent() ? 'accepted' : 'pending');

  useEffect(() => {
    if (consent !== 'accepted') document.documentElement.dataset.theme = 'dark';
  }, [consent]);

  if (consent === 'accepted') return <ReminderApp />;

  return (
    <ConsentGate
      state={consent}
      onCancel={() => setConsent('declined')}
      onReview={() => setConsent('pending')}
      onContinue={() => {
        try {
          localStorage.setItem(consentStorageKey, 'accepted');
          setConsent('accepted');
          return null;
        } catch {
          return "ui.this_choice_could_not_be_saved_locally_check_the_app_storage_permissions_and_try_again";
        }
      }}
    />
  );
}
