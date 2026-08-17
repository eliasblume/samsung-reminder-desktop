async function extensionRuntime(operation, args) {
  const baseUrl = 'https://api.samsungcloud.com';
  const appId = '8o8b82h22a';
  const table = 'com.samsung.android.app.reminder';

  function credential(method, options) {
    return new Promise((resolve, reject) => {
      method(options, (value) => {
        const error = chrome.runtime.lastError?.message;
        if (error || !value) reject(new Error('Credential unavailable'));
        else resolve(value);
      });
    });
  }

  async function context() {
    if (!chrome.credentials) throw new Error('Samsung private credentials API is unavailable');
    const accessToken = await credential(
      chrome.credentials.getAccessToken.bind(chrome.credentials),
      { provider: 'samsung' },
    );
    const accountInfo = await credential(
      chrome.credentials.getAccountInfo.bind(chrome.credentials),
      { provider: 'samsung' },
    );
    const stored = await new Promise((resolve) => {
      chrome.storage.local.get('account-storage', resolve);
    });
    const accountState = JSON.parse(stored['account-storage'] || '{}');
    const samsungAccount = accountState.state?.accounts?.find(
      (account) => account.type === 'samsung',
    );
    let deviceId = samsungAccount?.dvcid || stored['reminder-bridge-device-id'];
    if (!deviceId) {
      deviceId = crypto.randomUUID();
      await new Promise((resolve) => {
        chrome.storage.local.set({ 'reminder-bridge-device-id': deviceId }, resolve);
      });
    }
    if (!accountInfo.identifiers?.userId) {
      throw new Error('Credential unavailable');
    }
    return {
      accessToken,
      userId: accountInfo.identifiers.userId,
      deviceId,
      accountEmail: accountInfo.identifiers.loginId || samsungAccount?.email || null,
      identityChecked: true,
    };
  }

  async function request(path, options = {}) {
    const auth = await context();
    const response = await fetch(`${baseUrl}${path}`, {
      ...options,
      headers: {
        'x-sc-uid': auth.userId,
        'x-sc-access-token': auth.accessToken,
        'x-sc-app-id': appId,
        'x-sc-dvc-id': auth.deviceId,
        ...(options.body ? { 'content-type': 'application/json' } : {}),
        ...options.headers,
      },
    });
    const text = await response.text();
    let body = {};
    try { body = text ? JSON.parse(text) : {}; } catch { body = {}; }
    if (!response.ok) {
      const code = body.rcode ?? body.code ?? body.errorCode ?? 'unknown';
      throw new Error(`Samsung Cloud HTTP ${response.status}, code ${code}`);
    }
    return { status: response.status, body };
  }

  if (operation === 'credential') {
    return context();
  }

  async function listRecordIds(limit) {
    const requested = Math.max(1, Math.min(Number(limit) || 100, 500));
    const params = new URLSearchParams({
      table_ver: '1',
      select: 'record_id,mod_timestamp',
      limit: String(requested),
      meta: 'true',
      include_deleted_items: 'false',
    });
    const result = await request(`/data/v2/${table}?${params}`);
    return {
      status: result.status,
      ids: (result.body.records || []).map((record) => record.record_id).filter(Boolean),
      meta: result.body.meta || {},
    };
  }

  async function getRecords(ids) {
    if (!ids.length) return [];
    const all = [];
    for (let index = 0; index < ids.length; index += 100) {
      const chunk = ids.slice(index, index + 100);
      const result = await request(`/data/v2/${table}/get?table_ver=1&meta=false`, {
        method: 'POST',
        body: JSON.stringify({ records: chunk }),
      });
      all.push(...(result.body.records || []));
    }
    return all;
  }

  async function getRecord(id) {
    const records = await getRecords([id]);
    return records.find((record) => record.record_id === id) || null;
  }

  function iso(value) {
    return Number.isFinite(Number(value)) && Number(value) > 0
      ? new Date(Number(value)).toISOString()
      : null;
  }

  function publicRecord(record) {
    const hasLocation = Number(record.location_transition_type) > 0
      || Number.isFinite(Number(record.location_latitude))
      || Boolean(record.location_address);
    return {
      id: record.record_id,
      title: record.title || '',
      text: String(record.plainText || '').replace(/\n$/, ''),
      completed: Number(record.item_status) === 2,
      itemStatus: Number(record.item_status),
      favorite: Number(record.favorite) === 1,
      categoryId: record.category_id || null,
      allDay: Number(record.all_day) === 1,
      earlyAlert: (() => {
        try {
          const raw = Number(record.all_day) === 1 ? record.allday_pre_notify : record.time_alarm_pre_notify;
          const value = JSON.parse(raw || '{}').pList?.[0];
          return value ? { offset: Number(value.val), unit: value.u, exactTime: value.e ?? null } : null;
        } catch { return null; }
      })(),
      reminderAt: iso(record.alarm_reminde_time),
      startsAt: iso(record.start_time),
      endsAt: iso(record.end_time),
      createdAt: iso(record.time_create),
      modifiedAt: iso(record.last_modified_time || record.mod_timestamp),
      hasLocation,
      locationAddress: record.location_address || null,
      url: record.url || null,
    };
  }

  function xmlEscape(value) {
    return String(value)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&apos;');
  }

  function contentsXml(text) {
    const contents = text ? `<TextItem>${xmlEscape(text)}</TextItem>` : '';
    return `<?xml version="1.0" encoding="UTF-8" standalone="no"?><content>${contents}</content>`;
  }

  function newRecord(title, text) {
    const now = Date.now();
    const record = {
      record_id: crypto.randomUUID(), mod_timestamp: now, event_type: 0,
      item_status: 1, item_color: 0, title, time_create: now,
      last_modified_time: now, root_reminder_record_id: null,
      category_id: 'LOCAL_SPACE', favorite: 0, weight: now,
      text: contentsXml(text), plainText: text ? `${text}\n` : '', utterance: '',
      has_checkbox: 0, has_attached_file: 0, alarm_repeat_type: null,
      alarm_repeat_weekdays: null, alarm_reminde_time: null, alarm_tpo_type: null,
      rrule: null, date_rrule: null,
      time_alarm_pre_notify: null, allday_pre_notify: null,
      time_alarm_notify_info: null, allday_alarm_notify_info: null,
      location_transition_type: null, location_latitude: null,
      location_longitude: null, location_address: null,
      location_place_of_interest: null, location_repeat_type: null,
      location_locality: null, unified_profile_type: null,
      unified_profile_name: null, radius: null, occasion_key: null,
      occasion_type: null, occasion_event_type: null,
      occasion_event_repeat_type: null, occasion_name: null, occasion_info1: null,
      occasion_info2: null, occasion_info3: null, during_option_start_time: null,
      during_option_end_time: null, time_dismissed: null, event_status: null,
      alarm_sound_type: null, alert_type: null, start_time: null, end_time: null,
      all_day: null, app_card_type: null, app_card_content_intent: null,
      app_card_info_1: null, app_card_info_2: null, app_card_info_3: null,
      web_title: null, web_description: null, web_thumbnail: null, url: null,
    };
    for (let index = 0; index < 8; index += 1) {
      record[`original_image_${index}`] = null;
      record[`original_image_${index}_position`] = null;
    }
    return record;
  }

  async function upload(record) {
    const params = new URLSearchParams({
      table_ver: '1', upsert: 'true', partial_update: 'true',
      condition: 'mod_timestamp lt mod_timestamp',
    });
    const result = await request(`/data/v2/${table}?${params}`, {
      method: 'PUT', body: JSON.stringify({ records: [record] }),
    });
    const failed = result.body.failed_records || [];
    if (failed.length) throw new Error(`Samsung Cloud rejected record, code ${failed[0].rcode ?? 'unknown'}`);
    return result.status;
  }

  if (operation === 'probe') {
    const listed = await listRecordIds(1);
    return {
      extensionId: chrome.runtime.id,
      credentialsApi: Boolean(chrome.credentials),
      credentialAvailable: true,
      reminderTableStatus: listed.status,
      reminderRecordAvailable: listed.ids.length > 0,
    };
  }
  if (operation === 'list') {
    const listed = await listRecordIds(args.limit);
    const records = await getRecords(listed.ids);
    return {
      count: records.length,
      reminders: records.map(publicRecord),
      hasMore: Boolean(listed.meta.next_offset),
    };
  }
  if (operation === 'get') {
    const record = await getRecord(args.id);
    if (!record) throw new Error('Reminder not found');
    return publicRecord(record);
  }
  if (operation === 'create') {
    const title = String(args.title || '').trim();
    if (!title) throw new Error('A non-empty title is required');
    const text = String(args.text || '');
    const record = newRecord(title, text);
    if (args.id) record.record_id = String(args.id);
    const uploadStatus = await upload(record);
    const saved = await getRecord(record.record_id);
    if (!saved || saved.title !== title || String(saved.plainText || '').replace(/\n$/, '') !== text) {
      throw new Error('Create verification failed');
    }
    return { uploadStatus, reminder: publicRecord(saved) };
  }
  if (operation === 'update') {
    const record = await getRecord(args.id);
    if (!record) throw new Error('Reminder not found');
    let changed = false;
    if (Object.hasOwn(args, 'title')) {
      const title = String(args.title || '').trim();
      if (!title) throw new Error('Title cannot be empty');
      record.title = title; changed = true;
    }
    if (Object.hasOwn(args, 'text')) {
      const text = String(args.text || '');
      record.text = contentsXml(text); record.plainText = text ? `${text}\n` : '';
      record.has_checkbox = 0; changed = true;
    }
    if (Object.hasOwn(args, 'completed')) {
      record.item_status = args.completed ? 2 : 1; changed = true;
    }
    if (Object.hasOwn(args, 'favorite')) {
      record.favorite = args.favorite ? 1 : 0; changed = true;
    }
    if (!changed) throw new Error('No update fields were supplied');
    const now = Date.now();
    record.mod_timestamp = now; record.last_modified_time = now;
    const uploadStatus = await upload(record);
    const saved = await getRecord(record.record_id);
    if (!saved) throw new Error('Update verification failed');
    if (Object.hasOwn(args, 'title') && saved.title !== record.title) throw new Error('Title update verification failed');
    if (Object.hasOwn(args, 'text') && String(saved.plainText || '').replace(/\n$/, '') !== String(args.text || '')) throw new Error('Text update verification failed');
    if (Object.hasOwn(args, 'completed') && (Number(saved.item_status) === 2) !== Boolean(args.completed)) throw new Error('Completion update verification failed');
    if (Object.hasOwn(args, 'favorite') && (Number(saved.favorite) === 1) !== Boolean(args.favorite)) throw new Error('Favorite update verification failed');
    return { uploadStatus, reminder: publicRecord(saved) };
  }
  if (operation === 'delete') {
    if (!args.id || args.confirmId !== args.id) throw new Error('Delete requires confirmId to exactly match id');
    const record = await getRecord(args.id);
    if (!record) throw new Error('Reminder not found');
    const params = new URLSearchParams({
      action: 'delete', table_ver: '1', condition: 'mod_timestamp lt mod_timestamp',
    });
    const result = await request(`/data/v2/${table}?${params}`, {
      method: 'POST',
      body: JSON.stringify({ records: [{ record_id: args.id, mod_timestamp: Date.now() }] }),
    });
    const failed = result.body.failed_records || [];
    if (failed.length) throw new Error(`Samsung Cloud rejected delete, code ${failed[0].rcode ?? 'unknown'}`);
    return { deleted: true, id: args.id, status: result.status };
  }
  throw new Error(`Unknown operation: ${operation}`);
}
