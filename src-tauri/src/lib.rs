#![recursion_limit = "256"]

pub mod bridge;
mod cloud;

use serde_json::{json, Value};

#[tauri::command]
async fn reminder_operation(
    operation: String,
    args: Value,
    endpoint: Option<String>,
) -> Result<Value, String> {
    bridge::run_managed_operation(
        &operation,
        args,
        endpoint.as_deref().unwrap_or(bridge::DEFAULT_ENDPOINT),
    )
    .await
}

#[tauri::command]
async fn clear_reminder_credential() -> Result<(), String> {
    bridge::clear_cached_credential().await
}

#[tauri::command]
fn mcp_info() -> Value {
    json!({
        "binary": "samsung-reminder-mcp.exe",
        "transport": "stdio",
        "tools": [
            "samsung_reminders_status",
            "samsung_reminders_list",
            "samsung_reminder_categories_list",
            "samsung_reminder_category_create",
            "samsung_reminder_category_update",
            "samsung_reminder_category_delete",
            "samsung_reminders_get",
            "samsung_reminders_create",
            "samsung_reminders_update",
            "samsung_reminders_delete"
        ]
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            reminder_operation,
            clear_reminder_credential,
            mcp_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running Reminder");
}
