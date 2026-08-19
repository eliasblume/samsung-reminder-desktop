use super::{
    number,
    reminders::{new_record, public_record},
    schedule::apply_extended_fields,
};
use serde_json::{json, Value};

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
