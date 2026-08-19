use serde_json::{json, Value};
use uuid::Uuid;

use super::{
    client::{CloudClient, CATEGORY_TABLE, REMINDER_TABLE},
    now_millis, number, required_string, CloudError,
};

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

pub(super) async fn list(cloud: &CloudClient, _args: &Value) -> Result<Value, CloudError> {
    let (_, ids, _) = cloud.list_table_record_ids(CATEGORY_TABLE, 100).await?;
    let records = cloud.get_table_records(CATEGORY_TABLE, &ids).await?;
    let categories = records.iter().map(public_category).collect::<Vec<_>>();
    Ok(json!({ "count": categories.len(), "categories": categories }))
}

pub(super) async fn create(cloud: &CloudClient, args: &Value) -> Result<Value, CloudError> {
    let name = required_string(args, "name", "A category name is required")?.trim();
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
        .ok_or_else(|| CloudError::Message("Category creation verification failed".into()))?;
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

pub(super) async fn update(cloud: &CloudClient, args: &Value) -> Result<Value, CloudError> {
    let id = required_string(args, "id", "A category ID is required")?;
    if id == "LOCAL_SPACE" {
        return Err(CloudError::Message(
            "My reminders is Samsung's protected default category".into(),
        ));
    }
    let mut record = cloud
        .get_table_record(CATEGORY_TABLE, id)
        .await?
        .ok_or_else(|| CloudError::Message("Category not found".into()))?;
    let object = record
        .as_object_mut()
        .ok_or_else(|| CloudError::Message("Samsung Cloud returned an invalid category".into()))?;
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

pub(super) async fn delete(cloud: &CloudClient, args: &Value) -> Result<Value, CloudError> {
    let id = required_string(args, "id", "A category ID is required")?;
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
    let (_, reminder_ids, _) = cloud.list_table_record_ids(REMINDER_TABLE, 500).await?;
    let mut moved = 0;
    for mut reminder in cloud
        .get_table_records(REMINDER_TABLE, &reminder_ids)
        .await?
    {
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
        cloud
            .upload_table(REMINDER_TABLE, &reminder, "record")
            .await?;
        moved += 1;
    }
    let status = cloud
        .delete_table_record(CATEGORY_TABLE, id, "category delete")
        .await?;
    Ok(json!({ "deleted": true, "id": id, "status": status, "movedReminders": moved }))
}
