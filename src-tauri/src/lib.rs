#![recursion_limit = "256"]

pub mod bridge;
mod cloud;
pub mod operations;

use operations::ReminderOperation;
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
    let tools = ReminderOperation::ALL.map(ReminderOperation::tool_name);
    json!({
        "binary": "samsung-reminder-mcp.exe",
        "transport": "stdio",
        "tools": tools
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
