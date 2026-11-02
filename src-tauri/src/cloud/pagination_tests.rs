use super::super::{categories, reminders, CloudCredential};
use super::CloudClient;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::Duration;

struct CloudServer {
    base_url: String,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    mutations: Arc<Mutex<Vec<Value>>>,
}

impl CloudServer {
    fn new(mode: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let mutations = Arc::new(Mutex::new(Vec::new()));
        let worker_mutations = mutations.clone();
        let worker = thread::spawn(move || {
            while !worker_stop.load(Ordering::Relaxed) {
                let (mut socket, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("fixture accept: {error}"),
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                let (header_end, content_length) = loop {
                    let mut chunk = [0; 4096];
                    let count = socket.read(&mut chunk).unwrap();
                    assert!(count > 0, "request ended before headers");
                    bytes.extend_from_slice(&chunk[..count]);
                    if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        break (end + 4, length);
                    }
                };
                while bytes.len() < header_end + content_length {
                    let mut chunk = [0; 4096];
                    let count = socket.read(&mut chunk).unwrap();
                    assert!(count > 0, "request ended before body");
                    bytes.extend_from_slice(&chunk[..count]);
                }
                let request = String::from_utf8_lossy(&bytes[..header_end]);
                let path = request.split_whitespace().nth(1).unwrap();
                let is_next = path.contains("offset=next%2F%2B%3F") || path.contains("offset=17");
                let (status, body) = if path.contains("/get?") {
                    let body: Value = serde_json::from_slice(&bytes[header_end..]).unwrap();
                    let records: Vec<Value> = body["records"].as_array().unwrap().iter().map(|id| {
                        json!({"record_id": id, "title": "Synthetic fixture", "item_status": if id == "new" {1} else {2}, "category_id": if id == "new" {"target-category"} else {"other-category"}})
                    }).collect();
                    (200, json!({"records": records}))
                } else if request.starts_with("POST ") || request.starts_with("PUT ") {
                    let body: Value = serde_json::from_slice(&bytes[header_end..]).unwrap();
                    worker_mutations
                        .lock()
                        .unwrap()
                        .push(json!({"path": path, "body": body}));
                    (200, json!({}))
                } else if is_next && mode == "failure" {
                    (500, json!({"code": "fixture_failure"}))
                } else if is_next {
                    let meta = if mode == "cycle" {
                        json!({"next_offset": "next/+?"})
                    } else {
                        json!({})
                    };
                    let records = if mode == "duplicate" {
                        json!([{"record_id":"old-2"}, {"record_id":"new"}])
                    } else {
                        json!([{"record_id":"new"}])
                    };
                    (200, json!({"records": records, "meta": meta}))
                } else {
                    let cursor = if mode == "numeric" {
                        json!(17)
                    } else {
                        json!("next/+?")
                    };
                    (
                        200,
                        json!({"records": [{"record_id": "old-1"}, {"record_id": "old-2"}], "meta": {"next_offset": cursor}}),
                    )
                };
                let body = body.to_string();
                write!(socket, "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        Self {
            base_url,
            stop,
            worker: Some(worker),
            mutations,
        }
    }

    fn client(&self) -> CloudClient {
        CloudClient {
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
            credential: CloudCredential {
                access_token: "fixture".into(),
                user_id: "fixture".into(),
                device_id: "fixture".into(),
                account_email: None,
                identity_checked: true,
            },
            base_url: self.base_url.clone(),
        }
    }
}

impl Drop for CloudServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.worker.take().unwrap().join().unwrap();
    }
}

#[tokio::test]
async fn complete_sync_includes_active_reminders_after_the_first_page() {
    let server = CloudServer::new("normal");
    let result = reminders::list(&server.client(), &json!({"limit": 2, "all": true}))
        .await
        .unwrap();
    assert_eq!(result["count"], 3);
    assert_eq!(result["reminders"][2]["completed"], false);
    assert_eq!(result["hasMore"], false);
}

#[tokio::test]
async fn bounded_list_exposes_a_cursor_without_fetching_everything() {
    let server = CloudServer::new("normal");
    let result = reminders::list(&server.client(), &json!({"limit": 2}))
        .await
        .unwrap();
    assert_eq!(result["count"], 2);
    assert_eq!(result["hasMore"], true);
    assert_eq!(result["nextOffset"], "next/+?");
}

#[tokio::test]
async fn bounded_list_accepts_and_encodes_the_next_cursor() {
    let server = CloudServer::new("normal");
    let result = reminders::list(&server.client(), &json!({"limit": 2, "offset": "next/+?"}))
        .await
        .unwrap();
    assert_eq!(result["count"], 1);
    assert_eq!(result["reminders"][0]["id"], "new");
}

#[tokio::test]
async fn complete_sync_surfaces_later_page_failures_instead_of_partial_success() {
    let server = CloudServer::new("failure");
    let result = reminders::list(&server.client(), &json!({"limit": 2, "all": true})).await;
    assert!(
        result.is_err(),
        "a partial collection must not count as successful sync"
    );
}

#[tokio::test]
async fn complete_sync_rejects_repeated_cursors() {
    let server = CloudServer::new("cycle");
    let result = reminders::list(&server.client(), &json!({"limit": 2, "all": true})).await;
    assert!(result.is_err(), "a repeating cursor must not loop forever");
}

#[tokio::test]
async fn category_deletion_moves_reminders_from_later_pages() {
    let server = CloudServer::new("normal");
    let result = categories::delete(
        &server.client(),
        &json!({"id": "target-category", "confirmId": "target-category"}),
    )
    .await
    .unwrap();
    assert_eq!(result["movedReminders"], 1);
    let mutations = server.mutations.lock().unwrap();
    assert_eq!(mutations.len(), 2);
    assert!(mutations[0]["path"]
        .as_str()
        .unwrap()
        .contains("upsert=true"));
    assert_eq!(mutations[0]["body"]["records"][0]["record_id"], "new");
    assert_eq!(
        mutations[0]["body"]["records"][0]["category_id"],
        "LOCAL_SPACE"
    );
    assert!(mutations[1]["path"]
        .as_str()
        .unwrap()
        .contains("action=delete"));
    assert_eq!(
        mutations[1]["body"]["records"][0]["record_id"],
        "target-category"
    );
}

#[tokio::test]
async fn category_listing_includes_categories_from_later_pages() {
    let server = CloudServer::new("normal");
    let result = categories::list(&server.client(), &json!({}))
        .await
        .unwrap();
    assert_eq!(result["count"], 3);
}

#[tokio::test]
async fn category_deletion_does_not_mutate_anything_when_a_later_page_fails() {
    let server = CloudServer::new("failure");
    let result = categories::delete(
        &server.client(),
        &json!({"id": "target-category", "confirmId": "target-category"}),
    )
    .await;
    assert!(result.is_err());
    assert!(server.mutations.lock().unwrap().is_empty());
}

#[tokio::test]
async fn complete_sync_accepts_numeric_offsets() {
    let server = CloudServer::new("numeric");
    let result = reminders::list(&server.client(), &json!({"limit": 2, "all": true}))
        .await
        .unwrap();
    assert_eq!(result["count"], 3);
}

#[tokio::test]
async fn complete_sync_deduplicates_overlapping_pages() {
    let server = CloudServer::new("duplicate");
    let result = reminders::list(&server.client(), &json!({"limit": 2, "all": true}))
        .await
        .unwrap();
    assert_eq!(result["count"], 3);
    assert_eq!(result["reminders"][2]["id"], "new");
}
