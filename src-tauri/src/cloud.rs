use chrono::{DateTime, Datelike, FixedOffset, NaiveDateTime, SecondsFormat, Utc};
use reqwest::{Client, Method, StatusCode};
use serde_json::{json, Map, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const BASE_URL: &str = "https://api.samsungcloud.com";
const APP_ID: &str = "8o8b82h22a";
const TABLE: &str = "com.samsung.android.app.reminder";
const CATEGORY_TABLE: &str = "com.samsung.android.app.reminder.category";

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CloudCredential {
    access_token: String,
    user_id: String,
    device_id: String,
    #[serde(default)]
    account_email: Option<String>,
    #[serde(default)]
    identity_checked: bool,
}

impl CloudCredential {
    pub(crate) fn needs_identity_refresh(&self) -> bool {
        !self.identity_checked
    }
}

#[derive(Debug)]
pub(crate) enum CloudError {
    Unauthorized,
    Message(String),
}

impl CloudError {
    pub(crate) fn message(self) -> String {
        match self {
            Self::Unauthorized => "Samsung Cloud rejected the cached credential.".into(),
            Self::Message(message) => message,
        }
    }
}

struct CloudClient {
    client: Client,
    credential: CloudCredential,
}

impl CloudClient {
    fn new(credential: CloudCredential) -> Result<Self, CloudError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|error| CloudError::Message(error.to_string()))?;
        Ok(Self { client, credential })
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<(u16, Value), CloudError> {
        let mut request = self
            .client
            .request(method, format!("{BASE_URL}{path}"))
            .header("x-sc-uid", &self.credential.user_id)
            .header("x-sc-access-token", &self.credential.access_token)
            .header("x-sc-app-id", APP_ID)
            .header("x-sc-dvc-id", &self.credential.device_id);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.map_err(|error| {
            CloudError::Message(format!("Samsung Cloud request failed: {error}"))
        })?;
        let status = response.status();
        let text = response.text().await.map_err(|error| {
            CloudError::Message(format!("Samsung Cloud response failed: {error}"))
        })?;
        let parsed = serde_json::from_str(&text).unwrap_or_else(|_| json!({}));
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err(CloudError::Unauthorized);
        }
        if !status.is_success() {
            let code = parsed
                .get("rcode")
                .or_else(|| parsed.get("code"))
                .or_else(|| parsed.get("errorCode"))
                .map(Value::to_string)
                .unwrap_or_else(|| "unknown".into());
            return Err(CloudError::Message(format!(
                "Samsung Cloud HTTP {}, code {code}",
                status.as_u16()
            )));
        }
        Ok((status.as_u16(), parsed))
    }

    async fn list_table_record_ids(
        &self,
        table: &str,
        limit: u64,
    ) -> Result<(u16, Vec<String>, Value), CloudError> {
        let limit = limit.clamp(1, 500);
        let path = format!(
            "/data/v2/{table}?table_ver=1&select=record_id%2Cmod_timestamp&limit={limit}&meta=true&include_deleted_items=false"
        );
        let (status, body) = self.request(Method::GET, &path, None).await?;
        let ids = body
            .get("records")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|record| record.get("record_id").and_then(Value::as_str))
            .map(str::to_owned)
            .collect();
        Ok((
            status,
            ids,
            body.get("meta").cloned().unwrap_or_else(|| json!({})),
        ))
    }

    async fn list_record_ids(&self, limit: u64) -> Result<(u16, Vec<String>, Value), CloudError> {
        self.list_table_record_ids(TABLE, limit).await
    }

    async fn get_table_records(
        &self,
        table: &str,
        ids: &[String],
    ) -> Result<Vec<Value>, CloudError> {
        let mut records = Vec::new();
        for chunk in ids.chunks(100) {
            let (_, body) = self
                .request(
                    Method::POST,
                    &format!("/data/v2/{table}/get?table_ver=1&meta=false"),
                    Some(json!({ "records": chunk })),
                )
                .await?;
            if let Some(returned) = body.get("records").and_then(Value::as_array) {
                records.extend(returned.iter().cloned());
            }
        }
        Ok(records)
    }

    async fn get_records(&self, ids: &[String]) -> Result<Vec<Value>, CloudError> {
        self.get_table_records(TABLE, ids).await
    }

    async fn get_table_record(&self, table: &str, id: &str) -> Result<Option<Value>, CloudError> {
        Ok(self
            .get_table_records(table, &[id.to_owned()])
            .await?
            .into_iter()
            .find(|record| record.get("record_id").and_then(Value::as_str) == Some(id)))
    }

    async fn get_record(&self, id: &str) -> Result<Option<Value>, CloudError> {
        self.get_table_record(TABLE, id).await
    }

    async fn upload_table(
        &self,
        table: &str,
        record: &Value,
        noun: &str,
    ) -> Result<u16, CloudError> {
        let path = format!(
            "/data/v2/{table}?table_ver=1&upsert=true&partial_update=true&condition=mod_timestamp%20lt%20mod_timestamp"
        );
        let (status, body) = self
            .request(Method::PUT, &path, Some(json!({ "records": [record] })))
            .await?;
        reject_failed_records(&body, noun)?;
        Ok(status)
    }

    async fn upload(&self, record: &Value) -> Result<u16, CloudError> {
        self.upload_table(TABLE, record, "record").await
    }

    async fn delete_table_record(
        &self,
        table: &str,
        id: &str,
        noun: &str,
    ) -> Result<u16, CloudError> {
        let path = format!(
            "/data/v2/{table}?action=delete&table_ver=1&condition=mod_timestamp%20lt%20mod_timestamp"
        );
        let (status, body) = self
            .request(
                Method::POST,
                &path,
                Some(json!({ "records": [{ "record_id": id, "mod_timestamp": now_millis() }] })),
            )
            .await?;
        reject_failed_records(&body, noun)?;
        Ok(status)
    }
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn number(value: Option<&Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().map(|number| number as i64))
            .or_else(|| value.as_f64().map(|number| number as i64))
            .or_else(|| value.as_str().and_then(|number| number.parse().ok()))
    })
}

fn flag(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_bool)
        .unwrap_or_else(|| number(value) == Some(1))
}

fn iso(value: Option<&Value>) -> Value {
    number(value)
        .filter(|millis| *millis > 0)
        .and_then(DateTime::<Utc>::from_timestamp_millis)
        .map(|date| Value::String(date.to_rfc3339_opts(SecondsFormat::Millis, true)))
        .unwrap_or(Value::Null)
}

fn parse_rrule_date(value: &str) -> Option<String> {
    NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ")
        .ok()
        .map(|date| DateTime::<Utc>::from_naive_utc_and_offset(date, Utc))
        .map(|date| date.to_rfc3339_opts(SecondsFormat::Secs, true))
}

fn recurrence(record: &Value) -> Value {
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

fn early_alert(record: &Value) -> Value {
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

fn expected_early_alert(value: &Value, all_day: bool) -> Result<Value, CloudError> {
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

fn apply_extended_fields(record: &mut Value, args: &Value) -> Result<(), CloudError> {
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

fn public_record(record: &Value) -> Value {
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

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&amp;", "&")
}

fn content_text(record: &Value) -> String {
    let contents = record
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if let Some(start) = contents.find("<TextItem>") {
        let content_start = start + "<TextItem>".len();
        if let Some(end) = contents[content_start..].find("</TextItem>") {
            return xml_unescape(&contents[content_start..content_start + end]);
        }
    }
    record
        .get("plainText")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim_end_matches('\n')
        .to_owned()
}

fn checklist_items(record: &Value) -> Vec<Value> {
    let mut remaining = record
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut items = Vec::new();
    while let Some(start) = remaining.find("<CheckItem") {
        remaining = &remaining[start..];
        let Some(tag_end) = remaining.find('>') else {
            break;
        };
        let attributes = &remaining[..tag_end];
        let content_start = tag_end + 1;
        let Some(content_end) = remaining[content_start..].find("</CheckItem>") else {
            break;
        };
        let content_end = content_start + content_end;
        items.push(json!({
            "text": xml_unescape(&remaining[content_start..content_end]),
            "done": attributes.contains("done=\"true\"")
        }));
        remaining = &remaining[content_end + "</CheckItem>".len()..];
    }
    items
}

fn contents_xml(text: &str, checklist: &[Value]) -> String {
    let checks = checklist
        .iter()
        .filter_map(|item| {
            let text = item.get("text")?.as_str()?.trim();
            if text.is_empty() {
                return None;
            }
            let done = item.get("done").and_then(Value::as_bool).unwrap_or(false);
            Some(format!(
                "<CheckItem done=\"{done}\">{}</CheckItem>",
                xml_escape(text)
            ))
        })
        .collect::<String>();
    let contents = if text.is_empty() {
        String::new()
    } else {
        format!("<TextItem>{}</TextItem>", xml_escape(text))
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?><content>{checks}{contents}</content>"
    )
}

fn content_plain_text(text: &str, checklist: &[Value]) -> String {
    let mut lines = checklist
        .iter()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if !text.is_empty() {
        lines.push(text.to_owned());
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

fn checklist_argument(args: &Value) -> Result<Option<Vec<Value>>, CloudError> {
    let Some(value) = args.get("checklist") else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(Some(Vec::new()));
    }
    let items = value
        .as_array()
        .ok_or_else(|| CloudError::Message("checklist must be an array or null".into()))?;
    let mut checked = Vec::new();
    for item in items {
        let text = item
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if text.is_empty() {
            continue;
        }
        checked.push(json!({
            "text": text,
            "done": item.get("done").and_then(Value::as_bool).unwrap_or(false)
        }));
    }
    Ok(Some(checked))
}

fn apply_content_fields(record: &mut Value, args: &Value) -> Result<bool, CloudError> {
    let checklist = checklist_argument(args)?;
    let has_text = args
        .as_object()
        .is_some_and(|args| args.contains_key("text"));
    if checklist.is_none() && !has_text {
        return Ok(false);
    }
    let text = args
        .get("text")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| content_text(record));
    let checklist = checklist.unwrap_or_else(|| checklist_items(record));
    let object = record
        .as_object_mut()
        .ok_or_else(|| CloudError::Message("Samsung Cloud returned an invalid reminder".into()))?;
    object.insert(
        "text".into(),
        Value::String(contents_xml(&text, &checklist)),
    );
    object.insert(
        "plainText".into(),
        Value::String(content_plain_text(&text, &checklist)),
    );
    object.insert(
        "has_checkbox".into(),
        json!(if checklist.is_empty() { 0 } else { 1 }),
    );
    Ok(true)
}

fn map_has_location(record: &Map<String, Value>) -> bool {
    number(record.get("location_transition_type")).unwrap_or(0) > 0
        || record
            .get("location_latitude")
            .is_some_and(|value| !value.is_null())
        || record
            .get("location_address")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty())
}

fn new_record(title: &str, text: &str) -> Value {
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

fn reject_failed_records(body: &Value, noun: &str) -> Result<(), CloudError> {
    let Some(failed) = body.get("failed_records").and_then(Value::as_array) else {
        return Ok(());
    };
    if let Some(first) = failed.first() {
        let code = first
            .get("rcode")
            .map(Value::to_string)
            .unwrap_or_else(|| "unknown".into());
        return Err(CloudError::Message(format!(
            "Samsung Cloud rejected {noun}, code {code}"
        )));
    }
    Ok(())
}

fn required_string<'a>(args: &'a Value, key: &str, message: &str) -> Result<&'a str, CloudError> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CloudError::Message(message.into()))
}

fn public_category(record: &Value) -> Value {
    json!({
        "id": record.get("record_id").and_then(Value::as_str).unwrap_or_default(),
        "name": record.get("name").and_then(Value::as_str).unwrap_or("Untitled"),
        "color": number(record.get("color")).unwrap_or(0),
        "iconIndex": number(record.get("icon_index")).unwrap_or(0),
        "order": number(record.get("order")).unwrap_or(0),
        "extensionInfo": record.get("extensionInfo").cloned().unwrap_or(Value::Null),
    })
}

fn masked_account_hint(user_id: &str) -> String {
    let suffix = user_id
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    if suffix.is_empty() {
        "Samsung account".into()
    } else {
        format!("Account ••••{suffix}")
    }
}

pub(crate) async fn run_operation(
    credential: CloudCredential,
    operation: &str,
    args: Value,
) -> Result<Value, CloudError> {
    let cloud = CloudClient::new(credential)?;
    match operation {
        "probe" => {
            let (status, ids, _) = cloud.list_record_ids(1).await?;
            Ok(json!({
                "credentialsApi": true,
                "credentialAvailable": true,
                "accountEmail": cloud.credential.account_email,
                "accountIdHint": masked_account_hint(&cloud.credential.user_id),
                "credentialStorage": "windows-credential-manager",
                "transport": "direct-after-hidden-bootstrap",
                "reminderTableStatus": status,
                "reminderRecordAvailable": !ids.is_empty()
            }))
        }
        "list" => {
            let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(100);
            let (_, ids, meta) = cloud.list_record_ids(limit).await?;
            let records = cloud.get_records(&ids).await?;
            Ok(json!({
                "count": records.len(),
                "reminders": records.iter().map(public_record).collect::<Vec<_>>(),
                "hasMore": meta.get("next_offset").is_some_and(|value| !value.is_null())
            }))
        }
        "list_categories" => {
            let (_, ids, _) = cloud.list_table_record_ids(CATEGORY_TABLE, 100).await?;
            let records = cloud.get_table_records(CATEGORY_TABLE, &ids).await?;
            let categories = records.iter().map(public_category).collect::<Vec<_>>();
            Ok(json!({ "count": categories.len(), "categories": categories }))
        }
        "create_category" => {
            let name = required_string(&args, "name", "A category name is required")?.trim();
            if name.is_empty() {
                return Err(CloudError::Message("A category name is required".into()));
            }
            let color = number(args.get("color")).unwrap_or(0).clamp(0, 7);
            let icon_index = number(args.get("iconIndex")).unwrap_or(1).clamp(0, 41);
            let order = number(args.get("order")).unwrap_or(0).max(0);
            let id = args
                .get("id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let record = json!({
                "record_id": id,
                "mod_timestamp": now_millis(),
                "name": name,
                "color": color,
                "icon_index": icon_index,
                "order": order,
                "extensionInfo": null,
            });
            let upload_status = cloud
                .upload_table(CATEGORY_TABLE, &record, "category")
                .await?;
            let saved = cloud
                .get_table_record(CATEGORY_TABLE, &id)
                .await?
                .ok_or_else(|| {
                    CloudError::Message("Category creation verification failed".into())
                })?;
            if saved.get("name").and_then(Value::as_str) != Some(name)
                || number(saved.get("color")) != Some(color)
                || number(saved.get("icon_index")) != Some(icon_index)
            {
                return Err(CloudError::Message(
                    "Category creation verification failed".into(),
                ));
            }
            Ok(json!({ "uploadStatus": upload_status, "category": public_category(&saved) }))
        }
        "update_category" => {
            let id = required_string(&args, "id", "A category ID is required")?;
            if id == "LOCAL_SPACE" {
                return Err(CloudError::Message(
                    "My reminders is Samsung's protected default category".into(),
                ));
            }
            let mut record = cloud
                .get_table_record(CATEGORY_TABLE, id)
                .await?
                .ok_or_else(|| CloudError::Message("Category not found".into()))?;
            let object = record.as_object_mut().ok_or_else(|| {
                CloudError::Message("Samsung Cloud returned an invalid category".into())
            })?;
            let mut changed = false;
            if let Some(value) = args.get("name") {
                let name = value.as_str().unwrap_or_default().trim();
                if name.is_empty() {
                    return Err(CloudError::Message("Category name cannot be empty".into()));
                }
                object.insert("name".into(), Value::String(name.into()));
                changed = true;
            }
            if let Some(color) = number(args.get("color")) {
                object.insert("color".into(), json!(color.clamp(0, 7)));
                changed = true;
            }
            if let Some(icon_index) = number(args.get("iconIndex")) {
                object.insert("icon_index".into(), json!(icon_index.clamp(0, 41)));
                changed = true;
            }
            if let Some(order) = number(args.get("order")) {
                object.insert("order".into(), json!(order.max(0)));
                changed = true;
            }
            if !changed {
                return Err(CloudError::Message(
                    "No category update fields were supplied".into(),
                ));
            }
            object.insert("mod_timestamp".into(), json!(now_millis()));
            let upload_status = cloud
                .upload_table(CATEGORY_TABLE, &record, "category")
                .await?;
            let saved = cloud
                .get_table_record(CATEGORY_TABLE, id)
                .await?
                .ok_or_else(|| CloudError::Message("Category update verification failed".into()))?;
            Ok(json!({ "uploadStatus": upload_status, "category": public_category(&saved) }))
        }
        "delete_category" => {
            let id = required_string(&args, "id", "A category ID is required")?;
            if id == "LOCAL_SPACE" {
                return Err(CloudError::Message(
                    "My reminders is Samsung's protected default category".into(),
                ));
            }
            if args.get("confirmId").and_then(Value::as_str) != Some(id) {
                return Err(CloudError::Message(
                    "Delete requires confirmId to exactly match the category ID".into(),
                ));
            }
            if cloud.get_table_record(CATEGORY_TABLE, id).await?.is_none() {
                return Err(CloudError::Message("Category not found".into()));
            }
            let (_, reminder_ids, _) = cloud.list_record_ids(500).await?;
            let mut moved = 0;
            for mut reminder in cloud.get_records(&reminder_ids).await? {
                if reminder.get("category_id").and_then(Value::as_str) != Some(id) {
                    continue;
                }
                let object = reminder.as_object_mut().ok_or_else(|| {
                    CloudError::Message("Samsung Cloud returned an invalid reminder".into())
                })?;
                let now = now_millis();
                object.insert("category_id".into(), Value::String("LOCAL_SPACE".into()));
                object.insert("mod_timestamp".into(), json!(now));
                object.insert("last_modified_time".into(), json!(now));
                cloud.upload(&reminder).await?;
                moved += 1;
            }
            let status = cloud
                .delete_table_record(CATEGORY_TABLE, id, "category delete")
                .await?;
            Ok(json!({ "deleted": true, "id": id, "status": status, "movedReminders": moved }))
        }
        "get" => {
            let id = required_string(&args, "id", "A reminder ID is required")?;
            cloud
                .get_record(id)
                .await?
                .as_ref()
                .map(public_record)
                .ok_or_else(|| CloudError::Message("Reminder not found".into()))
        }
        "create" => {
            let title = required_string(&args, "title", "A non-empty title is required")?.trim();
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
            apply_content_fields(&mut record, &args)?;
            apply_extended_fields(&mut record, &args)?;
            let id = record["record_id"].as_str().unwrap_or_default().to_owned();
            let upload_status = cloud.upload(&record).await?;
            let saved = cloud
                .get_record(&id)
                .await?
                .ok_or_else(|| CloudError::Message("Create verification failed".into()))?;
            if saved.get("title").and_then(Value::as_str) != Some(title)
                || content_text(&saved) != text
            {
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
        "update" => {
            let id = required_string(&args, "id", "A reminder ID is required")?;
            let mut record = cloud
                .get_record(id)
                .await?
                .ok_or_else(|| CloudError::Message("Reminder not found".into()))?;
            let mut changed = apply_content_fields(&mut record, &args)?;
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
            apply_extended_fields(&mut record, &args)?;
            changed |= extended_changed;
            if !changed {
                return Err(CloudError::Message("No update fields were supplied".into()));
            }
            let upload_status = cloud.upload(&record).await?;
            let saved = cloud
                .get_record(id)
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
        "delete" => {
            let id = required_string(&args, "id", "A reminder ID is required")?;
            if args.get("confirmId").and_then(Value::as_str) != Some(id) {
                return Err(CloudError::Message(
                    "Delete requires confirmId to exactly match id".into(),
                ));
            }
            if cloud.get_record(id).await?.is_none() {
                return Err(CloudError::Message("Reminder not found".into()));
            }
            let status = cloud.delete_table_record(TABLE, id, "delete").await?;
            Ok(json!({ "deleted": true, "id": id, "status": status }))
        }
        _ => Err(CloudError::Message(format!(
            "Unsupported Reminder operation: {operation}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_day_schedule_uses_samsung_dates_fields() {
        let mut record = new_record("All day", "");
        apply_extended_fields(
            &mut record,
            &json!({
                "reminderAt": "2030-08-18T00:00:00.000Z",
                "allDay": true,
                "repeat": { "unit": "month", "interval": 1, "byMonthDay": 18 }
            }),
        )
        .expect("all-day fields should be valid");

        assert_eq!(number(record.get("all_day")), Some(1));
        assert_eq!(number(record.get("start_time")), Some(1_913_241_600_000));
        assert_eq!(number(record.get("end_time")), Some(1_913_328_000_000));
        assert!(record.get("alarm_reminde_time").is_some_and(Value::is_null));
        assert!(record.get("rrule").is_some_and(Value::is_null));
        assert_eq!(
            record.get("date_rrule").and_then(Value::as_str),
            Some("FREQ=MONTHLY;INTERVAL=1;BYMONTHDAY=18;WKST=SU")
        );
        assert_eq!(number(record.get("event_type")), Some(0));

        let public = public_record(&record);
        assert_eq!(public.get("allDay").and_then(Value::as_bool), Some(true));
        assert_eq!(
            public.pointer("/repeat/unit").and_then(Value::as_str),
            Some("month")
        );
    }

    #[test]
    fn all_day_with_place_uses_combined_event_type() {
        let mut record = new_record("All day somewhere", "");
        apply_extended_fields(
            &mut record,
            &json!({
                "reminderAt": "2030-08-18T00:00:00.000Z",
                "allDay": true,
                "location": {
                    "latitude": 52.52,
                    "longitude": 13.405,
                    "address": "Berlin"
                }
            }),
        )
        .expect("combined fields should be valid");

        assert_eq!(number(record.get("event_type")), Some(9));
        assert_eq!(number(record.get("all_day")), Some(1));
    }

    #[test]
    fn all_day_early_alert_matches_samsung_payload() {
        let mut record = new_record("Early alert example", "");
        apply_extended_fields(
            &mut record,
            &json!({
                "reminderAt": "2030-08-18T00:00:00.000Z",
                "allDay": true,
                "earlyAlert": { "offset": 2, "unit": "d", "exactTime": 67 }
            }),
        )
        .expect("early alert fields should be valid");

        assert!(record
            .get("time_alarm_pre_notify")
            .is_some_and(Value::is_null));
        assert_eq!(
            record.get("allday_pre_notify").and_then(Value::as_str),
            Some(r#"{"pList":[{"e":67,"u":"d","val":2,"ver":1}]}"#)
        );
        let public = public_record(&record);
        assert_eq!(
            public.pointer("/earlyAlert/offset").and_then(Value::as_i64),
            Some(2)
        );
        assert_eq!(
            public.pointer("/earlyAlert/unit").and_then(Value::as_str),
            Some("d")
        );
        assert_eq!(
            public
                .pointer("/earlyAlert/exactTime")
                .and_then(Value::as_i64),
            Some(67)
        );
    }

    #[test]
    fn early_alert_moves_between_timed_and_all_day_fields() {
        let mut record = new_record("Move fields", "");
        apply_extended_fields(
            &mut record,
            &json!({
                "reminderAt": "2030-08-18T09:30:00.000Z",
                "earlyAlert": { "offset": 1, "unit": "h", "exactTime": null }
            }),
        )
        .expect("timed alert should be valid");
        assert!(record
            .get("time_alarm_pre_notify")
            .and_then(Value::as_str)
            .is_some());

        apply_extended_fields(&mut record, &json!({ "allDay": true }))
            .expect("all-day migration should be valid");
        assert!(record
            .get("time_alarm_pre_notify")
            .is_some_and(Value::is_null));
        assert!(record
            .get("allday_pre_notify")
            .and_then(Value::as_str)
            .is_some());
    }
}
