use chrono::{DateTime, Datelike, FixedOffset, NaiveDateTime, SecondsFormat, Utc};
use serde_json::{json, Value};

use super::{flag, map_has_location, now_millis, number, CloudError};

fn parse_rrule_date(value: &str) -> Option<String> {
    NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ")
        .ok()
        .map(|date| DateTime::<Utc>::from_naive_utc_and_offset(date, Utc))
        .map(|date| date.to_rfc3339_opts(SecondsFormat::Secs, true))
}

pub(super) fn recurrence(record: &Value) -> Value {
    let rrule = record
        .get("rrule")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .or_else(|| record.get("date_rrule").and_then(Value::as_str))
        .unwrap_or_default();
    if !rrule.is_empty() {
        let parts = rrule
            .split(';')
            .filter_map(|part| part.split_once('='))
            .collect::<std::collections::HashMap<_, _>>();
        let unit = match parts.get("FREQ").copied().unwrap_or_default() {
            "MINUTELY" => "minute",
            "HOURLY" => "hour",
            "DAILY" => "day",
            "WEEKLY" => "week",
            "MONTHLY" => "month",
            "YEARLY" => "year",
            _ => "day",
        };
        let interval = parts
            .get("INTERVAL")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(1);
        let count = parts
            .get("COUNT")
            .and_then(|value| value.parse::<u64>().ok());
        let until = parts.get("UNTIL").and_then(|value| parse_rrule_date(value));
        return json!({
            "unit": unit,
            "interval": interval,
            "count": count,
            "until": until,
            "byDay": parts.get("BYDAY").copied(),
            "byMonthDay": parts.get("BYMONTHDAY").and_then(|value| value.parse::<i64>().ok()),
            "byMonth": parts.get("BYMONTH").and_then(|value| value.parse::<i64>().ok()),
            "rrule": rrule,
        });
    }

    let repeat_type = number(record.get("alarm_repeat_type")).unwrap_or(0);
    let unit = match repeat_type {
        1 => Some("day"),
        2 => Some("week"),
        3 => Some("month"),
        4 => Some("year"),
        6 => Some("minute"),
        7 => Some("hour"),
        _ => None,
    };
    unit.map(
        |unit| json!({ "unit": unit, "interval": 1, "count": null, "until": null, "rrule": null }),
    )
    .unwrap_or(Value::Null)
}

pub(super) fn early_alert(record: &Value) -> Value {
    let field = if flag(record.get("all_day")) {
        "allday_pre_notify"
    } else {
        "time_alarm_pre_notify"
    };
    let Some(raw) = record.get(field) else {
        return Value::Null;
    };
    let parsed = match raw {
        Value::String(raw) => match serde_json::from_str::<Value>(raw) {
            Ok(parsed) => parsed,
            Err(_) => return Value::Null,
        },
        Value::Object(_) => raw.clone(),
        _ => return Value::Null,
    };
    let Some(item) = parsed.pointer("/pList/0") else {
        return Value::Null;
    };
    let offset = number(item.get("val")).unwrap_or(0);
    let unit = item.get("u").and_then(Value::as_str).unwrap_or_default();
    if offset < 1 || !matches!(unit, "m" | "h" | "d" | "w" | "mo" | "y") {
        return Value::Null;
    }
    json!({
        "offset": offset,
        "unit": unit,
        "exactTime": number(item.get("e")),
    })
}

fn serialize_early_alert(value: &Value) -> Result<Value, CloudError> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    let object = value
        .as_object()
        .ok_or_else(|| CloudError::Message("earlyAlert must be an object or null".into()))?;
    let offset = object
        .get("offset")
        .and_then(Value::as_i64)
        .unwrap_or(1)
        .clamp(1, 999);
    let unit = object.get("unit").and_then(Value::as_str).unwrap_or("d");
    if !matches!(unit, "m" | "h" | "d" | "w" | "mo" | "y") {
        return Err(CloudError::Message(
            "earlyAlert.unit must be m, h, d, w, mo, or y".into(),
        ));
    }
    let mut item = json!({ "val": offset, "u": unit, "ver": 1 });
    if let Some(exact_time) = object.get("exactTime").and_then(Value::as_i64) {
        item.as_object_mut()
            .expect("early alert item is an object")
            .insert("e".into(), json!(exact_time.clamp(-1439, 1439)));
    }
    Ok(Value::String(json!({ "pList": [item] }).to_string()))
}

pub(super) fn expected_early_alert(value: &Value, all_day: bool) -> Result<Value, CloudError> {
    let field = if all_day {
        "allday_pre_notify"
    } else {
        "time_alarm_pre_notify"
    };
    let mut record = json!({ "all_day": if all_day { 1 } else { 0 } });
    record[field] = serialize_early_alert(value)?;
    Ok(early_alert(&record))
}

fn parse_schedule(value: &Value) -> Result<Option<DateTime<FixedOffset>>, CloudError> {
    if value.is_null() || value.as_str().is_some_and(str::is_empty) {
        return Ok(None);
    }
    let value = value
        .as_str()
        .ok_or_else(|| CloudError::Message("reminderAt must be an ISO date or null".into()))?;
    DateTime::parse_from_rfc3339(value)
        .map(Some)
        .map_err(|_| CloudError::Message("reminderAt must be a valid ISO date".into()))
}

fn build_rrule(repeat: &Value, start: DateTime<FixedOffset>) -> Result<Option<String>, CloudError> {
    if repeat.is_null() {
        return Ok(None);
    }
    let unit = repeat.get("unit").and_then(Value::as_str).unwrap_or("none");
    if unit == "none" {
        return Ok(None);
    }
    let frequency = match unit {
        "minute" => "MINUTELY",
        "hour" => "HOURLY",
        "day" => "DAILY",
        "week" => "WEEKLY",
        "month" => "MONTHLY",
        "year" => "YEARLY",
        _ => return Err(CloudError::Message("Unsupported repeat unit".into())),
    };
    let interval = repeat
        .get("interval")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .clamp(1, 999);
    let mut parts = vec![format!("FREQ={frequency}"), format!("INTERVAL={interval}")];
    match unit {
        "week" => {
            const DAYS: [&str; 7] = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];
            let by_day = repeat
                .get("byDay")
                .and_then(Value::as_str)
                .filter(|value| DAYS.contains(value))
                .unwrap_or(DAYS[start.weekday().num_days_from_monday() as usize]);
            parts.push(format!("BYDAY={}", by_day));
        }
        "month" => parts.push(format!(
            "BYMONTHDAY={}",
            repeat
                .get("byMonthDay")
                .and_then(Value::as_i64)
                .unwrap_or(start.day() as i64)
                .clamp(1, 31)
        )),
        "year" => {
            parts.push(format!(
                "BYMONTH={}",
                repeat
                    .get("byMonth")
                    .and_then(Value::as_i64)
                    .unwrap_or(start.month() as i64)
                    .clamp(1, 12)
            ));
            parts.push(format!(
                "BYMONTHDAY={}",
                repeat
                    .get("byMonthDay")
                    .and_then(Value::as_i64)
                    .unwrap_or(start.day() as i64)
                    .clamp(1, 31)
            ));
        }
        _ => {}
    }
    if let Some(count) = repeat.get("count").and_then(Value::as_u64) {
        parts.push(format!("COUNT={}", count.clamp(1, 9999)));
    } else if let Some(until) = repeat.get("until").and_then(Value::as_str) {
        let until = DateTime::parse_from_rfc3339(until)
            .map_err(|_| CloudError::Message("repeat.until must be a valid ISO date".into()))?
            .with_timezone(&Utc)
            .format("%Y%m%dT%H%M%SZ")
            .to_string();
        parts.push(format!("UNTIL={until}"));
    }
    parts.push("WKST=SU".into());
    Ok(Some(parts.join(";")))
}

pub(super) fn apply_extended_fields(record: &mut Value, args: &Value) -> Result<(), CloudError> {
    let object = record
        .as_object_mut()
        .ok_or_else(|| CloudError::Message("Samsung Cloud returned an invalid reminder".into()))?;
    let arguments = args
        .as_object()
        .ok_or_else(|| CloudError::Message("Reminder arguments must be an object".into()))?;
    let had_location = map_has_location(object);

    if let Some(category_id) = arguments.get("categoryId") {
        let category_id = category_id
            .as_str()
            .filter(|value| !value.is_empty())
            .unwrap_or("LOCAL_SPACE");
        object.insert("category_id".into(), Value::String(category_id.into()));
    }
    if let Some(alert_type) = arguments.get("alertType").and_then(Value::as_i64) {
        if !matches!(alert_type, 0 | 16 | 17) {
            return Err(CloudError::Message(
                "alertType must be 0 (weak), 16 (medium), or 17 (strong)".into(),
            ));
        }
        object.insert("alert_type".into(), json!(alert_type));
        object.insert("alarm_sound_type".into(), json!(alert_type & 1));
    }

    let schedule_was_supplied = arguments.contains_key("reminderAt");
    let all_day_was_supplied = arguments.contains_key("allDay");
    let all_day = arguments
        .get("allDay")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| flag(object.get("all_day")));
    let schedule = if let Some(value) = arguments.get("reminderAt") {
        parse_schedule(value)?
    } else {
        let stored_schedule = if all_day {
            number(object.get("start_time")).or_else(|| number(object.get("alarm_reminde_time")))
        } else {
            number(object.get("alarm_reminde_time")).or_else(|| number(object.get("start_time")))
        };
        stored_schedule.and_then(|millis| {
            DateTime::<Utc>::from_timestamp_millis(millis)
                .and_then(|date| DateTime::parse_from_rfc3339(&date.to_rfc3339()).ok())
        })
    };
    let has_schedule = schedule.is_some();

    if schedule_was_supplied && schedule.is_none() {
        for key in [
            "alarm_reminde_time",
            "alarm_repeat_type",
            "alarm_repeat_weekdays",
            "alarm_tpo_type",
            "rrule",
            "date_rrule",
            "start_time",
            "end_time",
            "all_day",
            "time_alarm_pre_notify",
            "allday_pre_notify",
        ] {
            object.insert(key.into(), Value::Null);
        }
        object.insert("event_type".into(), json!(if had_location { 5 } else { 0 }));
        object.insert(
            "event_status".into(),
            json!(if had_location { 1 } else { 0 }),
        );
    } else if let Some(schedule) = schedule {
        let rrule = if let Some(repeat) = arguments.get("repeat") {
            build_rrule(repeat, schedule)?
        } else if all_day {
            object
                .get("date_rrule")
                .and_then(Value::as_str)
                .or_else(|| object.get("rrule").and_then(Value::as_str))
                .map(str::to_owned)
        } else {
            object
                .get("rrule")
                .and_then(Value::as_str)
                .or_else(|| object.get("date_rrule").and_then(Value::as_str))
                .map(str::to_owned)
        };
        let repeats = rrule.is_some();
        if all_day {
            let date = schedule.date_naive();
            let start = date
                .and_hms_opt(0, 0, 0)
                .map(|value| DateTime::<Utc>::from_naive_utc_and_offset(value, Utc))
                .ok_or_else(|| CloudError::Message("All-day date is invalid".into()))?
                .timestamp_millis();
            object.insert("start_time".into(), json!(start));
            object.insert("end_time".into(), json!(start + 86_400_000));
            object.insert("all_day".into(), json!(1));
            object.insert("alarm_reminde_time".into(), Value::Null);
            object.insert("alarm_repeat_type".into(), Value::Null);
            object.insert("alarm_repeat_weekdays".into(), Value::Null);
            object.insert("alarm_tpo_type".into(), Value::Null);
            object.insert("rrule".into(), Value::Null);
            if all_day_was_supplied && !arguments.contains_key("earlyAlert") {
                let existing = object
                    .get("time_alarm_pre_notify")
                    .cloned()
                    .unwrap_or(Value::Null);
                object.insert("allday_pre_notify".into(), existing);
            }
            object.insert("time_alarm_pre_notify".into(), Value::Null);
            object.insert(
                "date_rrule".into(),
                rrule.map(Value::String).unwrap_or(Value::Null),
            );
            object.insert("event_type".into(), json!(if had_location { 9 } else { 0 }));
            object.insert(
                "event_status".into(),
                json!(if repeats || start + 86_400_000 > now_millis() {
                    1
                } else {
                    3
                }),
            );
        } else {
            let millis = schedule.timestamp_millis();
            object.insert("alarm_reminde_time".into(), json!(millis));
            object.insert("alarm_repeat_weekdays".into(), json!(0));
            object.insert("alarm_tpo_type".into(), json!(0));
            object.insert("time_dismissed".into(), json!(0));
            object.insert("start_time".into(), Value::Null);
            object.insert("end_time".into(), Value::Null);
            object.insert("all_day".into(), Value::Null);
            object.insert("date_rrule".into(), Value::Null);
            if all_day_was_supplied && !arguments.contains_key("earlyAlert") {
                let existing = object
                    .get("allday_pre_notify")
                    .cloned()
                    .unwrap_or(Value::Null);
                object.insert("time_alarm_pre_notify".into(), existing);
            }
            object.insert("allday_pre_notify".into(), Value::Null);
            object.insert(
                "event_status".into(),
                json!(if repeats || millis > now_millis() {
                    1
                } else {
                    3
                }),
            );
            if let Some(rrule) = rrule {
                object.insert("rrule".into(), Value::String(rrule));
                object.insert("alarm_repeat_type".into(), json!(5));
                object.insert("event_type".into(), json!(if had_location { 8 } else { 4 }));
            } else {
                object.insert("rrule".into(), Value::Null);
                object.insert("alarm_repeat_type".into(), json!(0));
                object.insert("event_type".into(), json!(if had_location { 8 } else { 1 }));
            }
        }
    } else if all_day_was_supplied {
        object.insert("all_day".into(), Value::Null);
    }

    if let Some(value) = arguments.get("earlyAlert") {
        if !has_schedule && !value.is_null() {
            return Err(CloudError::Message(
                "earlyAlert requires a scheduled reminder".into(),
            ));
        }
        let serialized = serialize_early_alert(value)?;
        let field = if all_day {
            "allday_pre_notify"
        } else {
            "time_alarm_pre_notify"
        };
        object.insert(field.into(), serialized);
        object.insert(
            (if all_day {
                "time_alarm_pre_notify"
            } else {
                "allday_pre_notify"
            })
            .into(),
            Value::Null,
        );
    }

    if let Some(location) = arguments.get("location") {
        if location.is_null() {
            for key in [
                "location_transition_type",
                "location_latitude",
                "location_longitude",
                "location_address",
                "location_place_of_interest",
                "location_repeat_type",
                "location_locality",
                "unified_profile_type",
                "unified_profile_name",
                "radius",
            ] {
                object.insert(key.into(), Value::Null);
            }
            let has_alarm = number(object.get("alarm_reminde_time")).unwrap_or(0) > 0;
            let has_dates = number(object.get("start_time")).unwrap_or(0) > 0;
            let has_repeat = object.get("rrule").and_then(Value::as_str).is_some();
            object.insert(
                "event_type".into(),
                json!(if has_alarm {
                    if has_repeat {
                        4
                    } else {
                        1
                    }
                } else {
                    0
                }),
            );
            if !has_alarm && !has_dates {
                object.insert("event_status".into(), json!(0));
            }
        } else {
            let location = location
                .as_object()
                .ok_or_else(|| CloudError::Message("location must be an object or null".into()))?;
            let latitude = location
                .get("latitude")
                .and_then(Value::as_f64)
                .ok_or_else(|| CloudError::Message("location.latitude is required".into()))?;
            let longitude = location
                .get("longitude")
                .and_then(Value::as_f64)
                .ok_or_else(|| CloudError::Message("location.longitude is required".into()))?;
            let address = location
                .get("address")
                .and_then(Value::as_str)
                .unwrap_or_default();
            object.insert(
                "location_transition_type".into(),
                json!(location
                    .get("transitionType")
                    .and_then(Value::as_i64)
                    .unwrap_or(1)),
            );
            object.insert("location_latitude".into(), json!(latitude));
            object.insert("location_longitude".into(), json!(longitude));
            object.insert("location_address".into(), Value::String(address.into()));
            object.insert(
                "location_place_of_interest".into(),
                location
                    .get("placeOfInterest")
                    .cloned()
                    .unwrap_or(Value::Null),
            );
            object.insert(
                "location_repeat_type".into(),
                json!(location
                    .get("repeatType")
                    .and_then(Value::as_i64)
                    .unwrap_or(10)),
            );
            object.insert(
                "unified_profile_type".into(),
                json!(location
                    .get("profileType")
                    .and_then(Value::as_i64)
                    .unwrap_or(0)),
            );
            object.insert(
                "unified_profile_name".into(),
                location.get("profileName").cloned().unwrap_or(Value::Null),
            );
            object.insert(
                "radius".into(),
                json!(location
                    .get("radius")
                    .and_then(Value::as_f64)
                    .unwrap_or(200.0)),
            );
            let has_alarm = number(object.get("alarm_reminde_time")).unwrap_or(0) > 0;
            let has_dates = number(object.get("start_time")).unwrap_or(0) > 0;
            object.insert(
                "event_type".into(),
                json!(if has_alarm {
                    8
                } else if has_dates {
                    9
                } else {
                    5
                }),
            );
            object.insert("event_status".into(), json!(1));
        }
    }
    Ok(())
}
