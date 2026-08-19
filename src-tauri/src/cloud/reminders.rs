use serde_json::{json, Value};
use uuid::Uuid;

use super::{
    client::{CloudClient, REMINDER_TABLE},
    content::{
        apply_content_fields, checklist_items, content_plain_text, content_text, contents_xml,
    },
    flag, iso, map_has_location, now_millis, number, required_string,
    schedule::{apply_extended_fields, early_alert, expected_early_alert, recurrence},
    CloudError,
};

pub(super) fn public_record(record: &Value) -> Value {
    let string = |key: &str| record.get(key).and_then(Value::as_str).unwrap_or_default();
    let has_location = record.as_object().is_some_and(map_has_location);
    let modified = record
        .get("last_modified_time")
        .filter(|value| number(Some(value)).unwrap_or(0) > 0)
        .or_else(|| record.get("mod_timestamp"));
    let checklist = checklist_items(record);
    json!({
        "id": string("record_id"),
        "title": string("title"),
        "text": content_text(record),
        "completed": number(record.get("item_status")) == Some(2),
        "itemStatus": number(record.get("item_status")).unwrap_or(0),
        "eventType": number(record.get("event_type")).unwrap_or(0),
        "eventStatus": number(record.get("event_status")).unwrap_or(0),
        "favorite": number(record.get("favorite")) == Some(1),
        "categoryId": record.get("category_id").cloned().unwrap_or(Value::Null),
        "allDay": flag(record.get("all_day")),
        "earlyAlert": early_alert(record),
        "reminderAt": iso(record.get("alarm_reminde_time")),
        "repeatType": number(record.get("alarm_repeat_type")).unwrap_or(0),
        "repeatWeekdays": number(record.get("alarm_repeat_weekdays")).unwrap_or(0),
        "rrule": record.get("rrule").filter(|value| !value.is_null()).cloned()
            .or_else(|| record.get("date_rrule").cloned()).unwrap_or(Value::Null),
        "repeat": recurrence(record),
        "tpoType": number(record.get("alarm_tpo_type")).unwrap_or(0),
        "soundType": number(record.get("alarm_sound_type")).unwrap_or(0),
        "alertType": number(record.get("alert_type")).unwrap_or(16),
        "hasCheckbox": !checklist.is_empty(),
        "checklist": checklist,
        "startsAt": iso(record.get("start_time")),
        "endsAt": iso(record.get("end_time")),
        "createdAt": iso(record.get("time_create")),
        "modifiedAt": iso(modified),
        "hasLocation": has_location,
        "locationAddress": record.get("location_address").cloned().unwrap_or(Value::Null),
        "location": if has_location { json!({
            "transitionType": number(record.get("location_transition_type")).unwrap_or(0),
            "latitude": record.get("location_latitude").cloned().unwrap_or(Value::Null),
            "longitude": record.get("location_longitude").cloned().unwrap_or(Value::Null),
            "address": record.get("location_address").cloned().unwrap_or(Value::Null),
            "placeOfInterest": record.get("location_place_of_interest").cloned().unwrap_or(Value::Null),
            "repeatType": number(record.get("location_repeat_type")).unwrap_or(0),
            "profileType": number(record.get("unified_profile_type")).unwrap_or(0),
            "profileName": record.get("unified_profile_name").cloned().unwrap_or(Value::Null),
            "radius": record.get("radius").cloned().unwrap_or(Value::Null),
        }) } else { Value::Null },
        "url": record.get("url").cloned().unwrap_or(Value::Null),
    })
}

pub(super) fn new_record(title: &str, text: &str) -> Value {
    let now = now_millis();
    let mut record = json!({
        "record_id": Uuid::new_v4().to_string(), "mod_timestamp": now, "event_type": 0,
        "item_status": 1, "item_color": 0, "title": title, "time_create": now,
        "last_modified_time": now, "root_reminder_record_id": null,
        "category_id": "LOCAL_SPACE", "favorite": 0, "weight": now,
        "text": contents_xml(text, &[]), "plainText": content_plain_text(text, &[]), "utterance": "",
        "has_checkbox": 0, "has_attached_file": 0, "alarm_repeat_type": null,
        "alarm_repeat_weekdays": null, "alarm_reminde_time": null, "alarm_tpo_type": null,
        "rrule": null, "date_rrule": null,
        "time_alarm_pre_notify": null, "allday_pre_notify": null,
        "time_alarm_notify_info": null, "allday_alarm_notify_info": null,
        "location_transition_type": null, "location_latitude": null,
        "location_longitude": null, "location_address": null,
        "location_place_of_interest": null, "location_repeat_type": null,
        "location_locality": null, "unified_profile_type": null,
        "unified_profile_name": null, "radius": null, "occasion_key": null,
        "occasion_type": null, "occasion_event_type": null,
        "occasion_event_repeat_type": null, "occasion_name": null,
        "occasion_info1": null, "occasion_info2": null, "occasion_info3": null,
        "during_option_start_time": null, "during_option_end_time": null,
        "time_dismissed": null, "event_status": null, "alarm_sound_type": null,
        "alert_type": null, "start_time": null, "end_time": null, "all_day": null,
        "app_card_type": null, "app_card_content_intent": null,
        "app_card_info_1": null, "app_card_info_2": null, "app_card_info_3": null,
        "web_title": null, "web_description": null, "web_thumbnail": null, "url": null
    });
    if let Some(object) = record.as_object_mut() {
        for index in 0..8 {
            object.insert(format!("original_image_{index}"), Value::Null);
            object.insert(format!("original_image_{index}_position"), Value::Null);
        }
    }
    record
}

pub(super) async fn list(cloud: &CloudClient, args: &Value) -> Result<Value, CloudError> {
    let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(100);
    let (_, ids, meta) = cloud.list_table_record_ids(REMINDER_TABLE, limit).await?;
    let records = cloud.get_table_records(REMINDER_TABLE, &ids).await?;
    Ok(json!({
        "count": records.len(),
        "reminders": records.iter().map(public_record).collect::<Vec<_>>(),
        "hasMore": meta.get("next_offset").is_some_and(|value| !value.is_null())
    }))
}

pub(super) async fn get(cloud: &CloudClient, args: &Value) -> Result<Value, CloudError> {
    let id = required_string(args, "id", "A reminder ID is required")?;
    cloud
        .get_table_record(REMINDER_TABLE, id)
        .await?
        .as_ref()
        .map(public_record)
        .ok_or_else(|| CloudError::Message("Reminder not found".into()))
}

pub(super) async fn create(cloud: &CloudClient, args: &Value) -> Result<Value, CloudError> {
    let title = required_string(args, "title", "A non-empty title is required")?.trim();
    if title.is_empty() {
        return Err(CloudError::Message("A non-empty title is required".into()));
    }
    let text = args.get("text").and_then(Value::as_str).unwrap_or_default();
    let mut record = new_record(title, text);
    if let Some(id) = args
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        record["record_id"] = Value::String(id.to_owned());
    }
    apply_content_fields(&mut record, args)?;
    apply_extended_fields(&mut record, args)?;
    let id = record["record_id"].as_str().unwrap_or_default().to_owned();
    let upload_status = cloud
        .upload_table(REMINDER_TABLE, &record, "record")
        .await?;
    let saved = cloud
        .get_table_record(REMINDER_TABLE, &id)
        .await?
        .ok_or_else(|| CloudError::Message("Create verification failed".into()))?;
    if saved.get("title").and_then(Value::as_str) != Some(title) || content_text(&saved) != text {
        return Err(CloudError::Message("Create verification failed".into()));
    }
    if let Some(value) = args.get("earlyAlert") {
        let expected = expected_early_alert(value, flag(saved.get("all_day")))?;
        if early_alert(&saved) != expected {
            return Err(CloudError::Message(
                "Samsung Cloud did not retain the early alert".into(),
            ));
        }
    }
    Ok(json!({ "uploadStatus": upload_status, "reminder": public_record(&saved) }))
}

pub(super) async fn update(cloud: &CloudClient, args: &Value) -> Result<Value, CloudError> {
    let id = required_string(args, "id", "A reminder ID is required")?;
    let mut record = cloud
        .get_table_record(REMINDER_TABLE, id)
        .await?
        .ok_or_else(|| CloudError::Message("Reminder not found".into()))?;
    let mut changed = apply_content_fields(&mut record, args)?;
    {
        let object = record.as_object_mut().ok_or_else(|| {
            CloudError::Message("Samsung Cloud returned an invalid reminder".into())
        })?;
        if let Some(value) = args.get("title") {
            let title = value.as_str().unwrap_or_default().trim();
            if title.is_empty() {
                return Err(CloudError::Message("Title cannot be empty".into()));
            }
            object.insert("title".into(), Value::String(title.into()));
            changed = true;
        }
        if let Some(value) = args.get("completed").and_then(Value::as_bool) {
            object.insert("item_status".into(), json!(if value { 2 } else { 1 }));
            changed = true;
        }
        if let Some(value) = args.get("favorite").and_then(Value::as_bool) {
            object.insert("favorite".into(), json!(if value { 1 } else { 0 }));
            changed = true;
        }
        let now = now_millis();
        object.insert("mod_timestamp".into(), json!(now));
        object.insert("last_modified_time".into(), json!(now));
    }
    let extended_changed = args.as_object().is_some_and(|arguments| {
        [
            "categoryId",
            "reminderAt",
            "allDay",
            "earlyAlert",
            "repeat",
            "alertType",
            "location",
        ]
        .iter()
        .any(|key| arguments.contains_key(*key))
    });
    apply_extended_fields(&mut record, args)?;
    changed |= extended_changed;
    if !changed {
        return Err(CloudError::Message("No update fields were supplied".into()));
    }
    let upload_status = cloud
        .upload_table(REMINDER_TABLE, &record, "record")
        .await?;
    let saved = cloud
        .get_table_record(REMINDER_TABLE, id)
        .await?
        .ok_or_else(|| CloudError::Message("Update verification failed".into()))?;
    if let Some(title) = args.get("title").and_then(Value::as_str) {
        if saved.get("title").and_then(Value::as_str) != Some(title.trim()) {
            return Err(CloudError::Message(
                "Title update verification failed".into(),
            ));
        }
    }
    if let Some(text) = args.get("text").and_then(Value::as_str) {
        if content_text(&saved) != text {
            return Err(CloudError::Message(
                "Text update verification failed".into(),
            ));
        }
    }
    if let Some(completed) = args.get("completed").and_then(Value::as_bool) {
        if (number(saved.get("item_status")) == Some(2)) != completed {
            return Err(CloudError::Message(
                "Completion update verification failed".into(),
            ));
        }
    }
    if let Some(favorite) = args.get("favorite").and_then(Value::as_bool) {
        if (number(saved.get("favorite")) == Some(1)) != favorite {
            return Err(CloudError::Message(
                "Favorite update verification failed".into(),
            ));
        }
    }
    if let Some(value) = args.get("earlyAlert") {
        let expected = expected_early_alert(value, flag(saved.get("all_day")))?;
        if early_alert(&saved) != expected {
            return Err(CloudError::Message(
                "Samsung Cloud did not retain the early alert".into(),
            ));
        }
    }
    Ok(json!({ "uploadStatus": upload_status, "reminder": public_record(&saved) }))
}

pub(super) async fn delete(cloud: &CloudClient, args: &Value) -> Result<Value, CloudError> {
    let id = required_string(args, "id", "A reminder ID is required")?;
    if args.get("confirmId").and_then(Value::as_str) != Some(id) {
        return Err(CloudError::Message(
            "Delete requires confirmId to exactly match id".into(),
        ));
    }
    if cloud.get_table_record(REMINDER_TABLE, id).await?.is_none() {
        return Err(CloudError::Message("Reminder not found".into()));
    }
    let status = cloud
        .delete_table_record(REMINDER_TABLE, id, "delete")
        .await?;
    Ok(json!({ "deleted": true, "id": id, "status": status }))
}
