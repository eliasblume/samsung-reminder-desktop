use samsung_reminder_desktop_lib::bridge;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};

fn tools() -> Value {
    let mut tools = json!([
        { "name": "samsung_reminders_status", "description": "Check the Samsung Browser credential bridge and Reminder table.", "inputSchema": { "type": "object", "additionalProperties": false, "properties": {} } },
        { "name": "samsung_reminders_list", "description": "List Samsung Reminder items.", "inputSchema": { "type": "object", "additionalProperties": false, "properties": { "limit": { "type": "integer", "minimum": 1, "maximum": 500, "default": 100 } } } },
        { "name": "samsung_reminder_categories_list", "description": "List Samsung Reminder custom categories.", "inputSchema": { "type": "object", "additionalProperties": false, "properties": {} } },
        { "name": "samsung_reminder_category_create", "description": "Create a Samsung Reminder list/category with a synced color and icon.", "inputSchema": { "type": "object", "additionalProperties": false, "required": ["name"], "properties": { "name": { "type": "string", "minLength": 1, "maxLength": 80 }, "color": { "type": "integer", "minimum": 0, "maximum": 7, "default": 0 }, "iconIndex": { "type": "integer", "minimum": 0, "maximum": 41, "default": 1 }, "order": { "type": "integer", "minimum": 0, "default": 0 } } } },
        { "name": "samsung_reminder_category_update", "description": "Rename, recolor, or change the synced icon of a Samsung Reminder list/category.", "inputSchema": { "type": "object", "additionalProperties": false, "required": ["id"], "properties": { "id": { "type": "string", "minLength": 1 }, "name": { "type": "string", "minLength": 1, "maxLength": 80 }, "color": { "type": "integer", "minimum": 0, "maximum": 7 }, "iconIndex": { "type": "integer", "minimum": 0, "maximum": 41 }, "order": { "type": "integer", "minimum": 0 } } } },
        { "name": "samsung_reminder_category_delete", "description": "Delete a Samsung Reminder list/category and move its reminders to My reminders. confirmId must exactly match id.", "inputSchema": { "type": "object", "additionalProperties": false, "required": ["id", "confirmId"], "properties": { "id": { "type": "string", "minLength": 1 }, "confirmId": { "type": "string", "minLength": 1 } } } },
        { "name": "samsung_reminders_get", "description": "Read one Samsung Reminder item by cloud ID.", "inputSchema": { "type": "object", "additionalProperties": false, "required": ["id"], "properties": { "id": { "type": "string", "minLength": 1 } } } },
        { "name": "samsung_reminders_create", "description": "Create a Samsung Reminder with optional notes, checklist, schedule, recurrence, place trigger, alert strength, and list.", "inputSchema": { "type": "object", "additionalProperties": false, "required": ["title"], "properties": { "title": { "type": "string", "minLength": 1 }, "text": { "type": "string", "default": "" }, "checklist": { "$ref": "#/$defs/checklist" }, "categoryId": { "type": "string" }, "reminderAt": { "type": ["string", "null"], "format": "date-time" }, "alertType": { "type": "integer", "enum": [0, 16, 17] }, "repeat": { "$ref": "#/$defs/repeat" }, "location": { "$ref": "#/$defs/location" } }, "$defs": { "checklist": { "type": "array", "items": { "type": "object", "additionalProperties": false, "required": ["text"], "properties": { "text": { "type": "string", "minLength": 1 }, "done": { "type": "boolean", "default": false } } } }, "repeat": { "type": ["object", "null"], "additionalProperties": false, "required": ["unit", "interval"], "properties": { "unit": { "type": "string", "enum": ["none", "minute", "hour", "day", "week", "month", "year"] }, "interval": { "type": "integer", "minimum": 1, "maximum": 999 }, "count": { "type": "integer", "minimum": 1, "maximum": 9999 }, "until": { "type": "string", "format": "date-time" }, "byDay": { "type": "string", "enum": ["MO", "TU", "WE", "TH", "FR", "SA", "SU"] }, "byMonthDay": { "type": "integer", "minimum": 1, "maximum": 31 }, "byMonth": { "type": "integer", "minimum": 1, "maximum": 12 } } }, "location": { "type": ["object", "null"], "additionalProperties": false, "required": ["latitude", "longitude"], "properties": { "latitude": { "type": "number", "minimum": -90, "maximum": 90 }, "longitude": { "type": "number", "minimum": -180, "maximum": 180 }, "address": { "type": "string" }, "placeOfInterest": { "type": ["string", "null"] }, "transitionType": { "type": "integer", "enum": [1, 2], "default": 1 }, "repeatType": { "type": "integer", "default": 10 }, "profileType": { "type": "integer", "default": 0 }, "profileName": { "type": ["string", "null"] }, "radius": { "type": "number", "minimum": 50, "maximum": 5000, "default": 200 } } } } } },
        { "name": "samsung_reminders_update", "description": "Update content, checklist, status, schedule, recurrence, place trigger, alert strength, or list.", "inputSchema": { "type": "object", "additionalProperties": false, "required": ["id"], "properties": { "id": { "type": "string", "minLength": 1 }, "title": { "type": "string", "minLength": 1 }, "text": { "type": "string" }, "checklist": { "$ref": "#/$defs/checklist" }, "completed": { "type": "boolean" }, "favorite": { "type": "boolean" }, "categoryId": { "type": "string" }, "reminderAt": { "type": ["string", "null"], "format": "date-time" }, "alertType": { "type": "integer", "enum": [0, 16, 17] }, "repeat": { "$ref": "#/$defs/repeat" }, "location": { "$ref": "#/$defs/location" } }, "$defs": { "checklist": { "type": "array", "items": { "type": "object", "additionalProperties": false, "required": ["text"], "properties": { "text": { "type": "string", "minLength": 1 }, "done": { "type": "boolean", "default": false } } } }, "repeat": { "type": ["object", "null"], "additionalProperties": false, "required": ["unit", "interval"], "properties": { "unit": { "type": "string", "enum": ["none", "minute", "hour", "day", "week", "month", "year"] }, "interval": { "type": "integer", "minimum": 1, "maximum": 999 }, "count": { "type": "integer", "minimum": 1, "maximum": 9999 }, "until": { "type": "string", "format": "date-time" }, "byDay": { "type": "string", "enum": ["MO", "TU", "WE", "TH", "FR", "SA", "SU"] }, "byMonthDay": { "type": "integer", "minimum": 1, "maximum": 31 }, "byMonth": { "type": "integer", "minimum": 1, "maximum": 12 } } }, "location": { "type": ["object", "null"], "additionalProperties": false, "required": ["latitude", "longitude"], "properties": { "latitude": { "type": "number", "minimum": -90, "maximum": 90 }, "longitude": { "type": "number", "minimum": -180, "maximum": 180 }, "address": { "type": "string" }, "placeOfInterest": { "type": ["string", "null"] }, "transitionType": { "type": "integer", "enum": [1, 2], "default": 1 }, "repeatType": { "type": "integer", "default": 10 }, "profileType": { "type": "integer", "default": 0 }, "profileName": { "type": ["string", "null"] }, "radius": { "type": "number", "minimum": 50, "maximum": 5000, "default": 200 } } } } } },
        { "name": "samsung_reminders_delete", "description": "Delete one reminder. confirmId must exactly match id.", "inputSchema": { "type": "object", "additionalProperties": false, "required": ["id", "confirmId"], "properties": { "id": { "type": "string", "minLength": 1 }, "confirmId": { "type": "string", "minLength": 1 } } } }
    ]);
    for index in [7_usize, 8] {
        tools[index]["inputSchema"]["properties"]["allDay"] = json!({
            "type": "boolean",
            "description": "Store reminderAt as a date-only all-day reminder."
        });
        tools[index]["inputSchema"]["properties"]["earlyAlert"] = json!({
            "type": ["object", "null"],
            "additionalProperties": false,
            "required": ["offset", "unit"],
            "properties": {
                "offset": { "type": "integer", "minimum": 1, "maximum": 999 },
                "unit": { "type": "string", "enum": ["m", "h", "d", "w", "mo", "y"] },
                "exactTime": { "type": ["integer", "null"], "minimum": -1439, "maximum": 1439 }
            }
        });
    }
    tools
}

fn operation_for_tool(name: &str) -> Option<&'static str> {
    match name {
        "samsung_reminders_status" => Some("probe"),
        "samsung_reminders_list" => Some("list"),
        "samsung_reminder_categories_list" => Some("list_categories"),
        "samsung_reminder_category_create" => Some("create_category"),
        "samsung_reminder_category_update" => Some("update_category"),
        "samsung_reminder_category_delete" => Some("delete_category"),
        "samsung_reminders_get" => Some("get"),
        "samsung_reminders_create" => Some("create"),
        "samsung_reminders_update" => Some("update"),
        "samsung_reminders_delete" => Some("delete"),
        _ => None,
    }
}

fn redact_status_identity(result: &mut Value) {
    let Some(status) = result.as_object_mut() else {
        return;
    };
    status.remove("accountEmail");
    status.remove("accountIdHint");
}

async fn handle(message: &Value) -> Option<Value> {
    let method = message.get("method")?.as_str()?;
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    match method {
        "notifications/initialized" => None,
        "initialize" => Some(json!({
            "jsonrpc": "2.0", "id": id,
            "result": {
                "protocolVersion": message.pointer("/params/protocolVersion").cloned().unwrap_or(json!("2025-03-26")),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "samsung-reminder-desktop", "version": env!("CARGO_PKG_VERSION") }
            }
        })),
        "tools/list" => Some(json!({ "jsonrpc": "2.0", "id": id, "result": { "tools": tools() } })),
        "tools/call" => {
            let name = message
                .pointer("/params/name")
                .and_then(Value::as_str)
                .unwrap_or("");
            let Some(operation) = operation_for_tool(name) else {
                return Some(
                    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32602, "message": format!("Unknown tool: {name}") } }),
                );
            };
            let args = message
                .pointer("/params/arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let endpoint =
                std::env::var("CDP_ENDPOINT").unwrap_or_else(|_| bridge::DEFAULT_ENDPOINT.into());
            match bridge::run_managed_operation(operation, args, &endpoint).await {
                Ok(mut result) => {
                    if name == "samsung_reminders_status" {
                        redact_status_identity(&mut result);
                    }
                    Some(json!({
                        "jsonrpc": "2.0", "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&result).unwrap_or_default() }],
                            "structuredContent": result
                        }
                    }))
                }
                Err(error) => Some(json!({
                    "jsonrpc": "2.0", "id": id,
                    "result": { "isError": true, "content": [{ "type": "text", "text": error }] }
                })),
            }
        }
        _ => message.get("id").map(|_| {
            json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {method}") }
            })
        }),
    }
}

#[tokio::main]
async fn main() {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(message) => handle(&message).await,
            Err(error) => Some(
                json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": error.to_string() } }),
            ),
        };
        if let Some(response) = response {
            println!("{response}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::redact_status_identity;
    use serde_json::json;

    #[test]
    fn removes_account_identity_from_mcp_status() {
        let mut status = json!({
            "accountEmail": "person@example.com",
            "accountIdHint": "••••1234",
            "credentialAvailable": true
        });
        redact_status_identity(&mut status);
        assert!(status.get("accountEmail").is_none());
        assert!(status.get("accountIdHint").is_none());
        assert_eq!(status["credentialAvailable"], true);
    }
}
