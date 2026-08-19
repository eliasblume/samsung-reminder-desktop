use super::CloudError;
use serde_json::{json, Value};

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

pub(super) fn content_text(record: &Value) -> String {
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

pub(super) fn checklist_items(record: &Value) -> Vec<Value> {
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

pub(super) fn contents_xml(text: &str, checklist: &[Value]) -> String {
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

pub(super) fn content_plain_text(text: &str, checklist: &[Value]) -> String {
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

pub(super) fn apply_content_fields(record: &mut Value, args: &Value) -> Result<bool, CloudError> {
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
