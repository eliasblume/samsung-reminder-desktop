use reqwest::{Client, Method, StatusCode};
use serde_json::{json, Value};

use super::{now_millis, CloudCredential, CloudError};

const BASE_URL: &str = "https://api.samsungcloud.com";
const APP_ID: &str = "8o8b82h22a";
pub(super) const REMINDER_TABLE: &str = "com.samsung.android.app.reminder";
pub(super) const CATEGORY_TABLE: &str = "com.samsung.android.app.reminder.category";

pub(super) struct CloudClient {
    client: Client,
    credential: CloudCredential,
}

impl CloudClient {
    pub(super) fn new(credential: CloudCredential) -> Result<Self, CloudError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|error| CloudError::Message(error.to_string()))?;
        Ok(Self { client, credential })
    }

    pub(super) fn credential(&self) -> &CloudCredential {
        &self.credential
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

    pub(super) async fn list_table_record_ids(
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

    pub(super) async fn list_record_ids(
        &self,
        limit: u64,
    ) -> Result<(u16, Vec<String>, Value), CloudError> {
        self.list_table_record_ids(REMINDER_TABLE, limit).await
    }

    pub(super) async fn get_table_records(
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

    pub(super) async fn get_table_record(
        &self,
        table: &str,
        id: &str,
    ) -> Result<Option<Value>, CloudError> {
        Ok(self
            .get_table_records(table, &[id.to_owned()])
            .await?
            .into_iter()
            .find(|record| record.get("record_id").and_then(Value::as_str) == Some(id)))
    }

    pub(super) async fn upload_table(
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

    pub(super) async fn delete_table_record(
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
