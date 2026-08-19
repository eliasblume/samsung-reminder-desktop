mod categories;
mod client;
mod content;
mod reminders;
mod schedule;

use crate::operations::ReminderOperation;
use chrono::{DateTime, SecondsFormat, Utc};
use client::CloudClient;
use serde_json::{json, Map, Value};
use std::time::{SystemTime, UNIX_EPOCH};

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

fn required_string<'a>(args: &'a Value, key: &str, message: &str) -> Result<&'a str, CloudError> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CloudError::Message(message.into()))
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
    operation: ReminderOperation,
    args: Value,
) -> Result<Value, CloudError> {
    let cloud = CloudClient::new(credential)?;
    match operation {
        ReminderOperation::Probe => {
            let (status, ids, _) = cloud.list_record_ids(1).await?;
            let credential = cloud.credential();
            Ok(json!({
                "credentialsApi": true,
                "credentialAvailable": true,
                "accountEmail": credential.account_email,
                "accountIdHint": masked_account_hint(&credential.user_id),
                "credentialStorage": "windows-credential-manager",
                "transport": "direct-after-hidden-bootstrap",
                "reminderTableStatus": status,
                "reminderRecordAvailable": !ids.is_empty()
            }))
        }
        ReminderOperation::List => reminders::list(&cloud, &args).await,
        ReminderOperation::ListCategories => categories::list(&cloud, &args).await,
        ReminderOperation::CreateCategory => categories::create(&cloud, &args).await,
        ReminderOperation::UpdateCategory => categories::update(&cloud, &args).await,
        ReminderOperation::DeleteCategory => categories::delete(&cloud, &args).await,
        ReminderOperation::Get => reminders::get(&cloud, &args).await,
        ReminderOperation::Create => reminders::create(&cloud, &args).await,
        ReminderOperation::Update => reminders::update(&cloud, &args).await,
        ReminderOperation::Delete => reminders::delete(&cloud, &args).await,
    }
}

#[cfg(test)]
mod tests;
