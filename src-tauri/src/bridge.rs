use crate::cloud::{self, CloudCredential, CloudError};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message};
#[cfg(target_os = "windows")]
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, ERROR_NOT_FOUND, HWND, INVALID_HANDLE_VALUE, LPARAM},
    Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
        CRED_TYPE_GENERIC,
    },
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
        Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE},
    },
    UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, ShowWindow, SW_HIDE},
};

pub const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:9226";
const CALENDAR_EXTENSION_ID: &str = "bhbojgnklikmfoopegjhgfbklnpbjboa";
const CALENDAR_PAGE: &str = "chrome-extension://bhbojgnklikmfoopegjhgfbklnpbjboa/src/index.html";
const CREDENTIAL_TARGET: &str = "SamsungReminderDesktop.SamsungCloud";
const EXTENSION_RUNTIME: &str = include_str!("../reminder-runtime.js");
static MANAGED_BROWSER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static CREDENTIAL_CACHE: tokio::sync::RwLock<Option<CloudCredential>> =
    tokio::sync::RwLock::const_new(None);

#[cfg(target_os = "windows")]
fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn load_persistent_credential() -> Result<Option<CloudCredential>, String> {
    let target = wide_null(CREDENTIAL_TARGET);
    let mut pointer: *mut CREDENTIALW = std::ptr::null_mut();
    if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut pointer) } == 0 {
        let error = unsafe { GetLastError() };
        if error == ERROR_NOT_FOUND {
            return Ok(None);
        }
        return Err(format!(
            "Could not read the Windows credential cache (error {error})"
        ));
    }
    if pointer.is_null() {
        return Ok(None);
    }
    let result = unsafe {
        let credential = &*pointer;
        let blob = std::slice::from_raw_parts(
            credential.CredentialBlob,
            credential.CredentialBlobSize as usize,
        );
        serde_json::from_slice::<CloudCredential>(blob)
            .map(Some)
            .map_err(|error| format!("The Windows credential cache is invalid: {error}"))
    };
    unsafe {
        CredFree(pointer.cast());
    }
    match result {
        Ok(credential) => Ok(credential),
        Err(_) => {
            let _ = delete_persistent_credential();
            Ok(None)
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn load_persistent_credential() -> Result<Option<CloudCredential>, String> {
    Ok(None)
}

#[cfg(target_os = "windows")]
fn store_persistent_credential(credential: &CloudCredential) -> Result<(), String> {
    let target = wide_null(CREDENTIAL_TARGET);
    let username = wide_null("Samsung Reminder");
    let mut blob = serde_json::to_vec(credential)
        .map_err(|error| format!("Could not encode the Samsung credential: {error}"))?;
    let value = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: target.as_ptr() as *mut u16,
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        UserName: username.as_ptr() as *mut u16,
        ..CREDENTIALW::default()
    };
    if unsafe { CredWriteW(&value, 0) } == 0 {
        let error = unsafe { GetLastError() };
        return Err(format!(
            "Could not save the Windows credential cache (error {error})"
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn store_persistent_credential(_credential: &CloudCredential) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
fn delete_persistent_credential() -> Result<(), String> {
    let target = wide_null(CREDENTIAL_TARGET);
    if unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) } == 0 {
        let error = unsafe { GetLastError() };
        if error != ERROR_NOT_FOUND {
            return Err(format!(
                "Could not clear the Windows credential cache (error {error})"
            ));
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn delete_persistent_credential() -> Result<(), String> {
    Ok(())
}

pub async fn clear_cached_credential() -> Result<(), String> {
    let _guard = MANAGED_BROWSER_LOCK.lock().await;
    *CREDENTIAL_CACHE.write().await = None;
    delete_persistent_credential()
}

#[derive(Debug, Deserialize)]
struct CdpTarget {
    #[serde(rename = "type")]
    target_type: String,
    url: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    websocket_debugger_url: Option<String>,
}

fn normalize_endpoint(endpoint: &str) -> Result<String, String> {
    let endpoint = reqwest::Url::parse(endpoint.trim())
        .map_err(|_| "The CDP endpoint must be http://127.0.0.1:<port>.".to_string())?;
    if endpoint.scheme() != "http"
        || endpoint.host_str() != Some("127.0.0.1")
        || endpoint.port().is_none()
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || !matches!(endpoint.path(), "" | "/")
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
    {
        return Err("The CDP endpoint must be http://127.0.0.1:<port>.".into());
    }
    Ok(format!(
        "http://127.0.0.1:{}",
        endpoint.port().expect("validated CDP port")
    ))
}

pub async fn cdp_available(endpoint: &str) -> bool {
    let Ok(endpoint) = normalize_endpoint(endpoint) else {
        return false;
    };
    reqwest::Client::new()
        .get(format!("{endpoint}/json/version"))
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .is_ok_and(|response| response.status().is_success())
}

fn endpoint_port(endpoint: &str) -> u16 {
    reqwest::Url::parse(endpoint)
        .ok()
        .and_then(|value| value.port())
        .unwrap_or(9226)
}

fn trusted_profile() -> Result<PathBuf, String> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| "Windows LocalAppData is unavailable.".to_string())?;
    let path = PathBuf::from(local_app_data)
        .join("Samsung")
        .join("Internet")
        .join("User Data");
    if !path.join("Default").is_dir() {
        return Err(
            "SAMSUNG_PROFILE_NOT_READY: Open Samsung Browser once so its profile can be initialized, then retry."
                .into(),
        );
    }
    Ok(path)
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn hide_window_for_pid(hwnd: HWND, lparam: LPARAM) -> i32 {
    let target_pid = unsafe { *(lparam as *const u32) };
    let mut window_pid = 0_u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut window_pid);
    }
    if window_pid == target_pid {
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
        }
    }
    1
}

#[cfg(target_os = "windows")]
fn hide_process_windows(pid: u32) {
    unsafe {
        EnumWindows(Some(hide_window_for_pid), &pid as *const u32 as LPARAM);
    }
}

#[cfg(target_os = "windows")]
fn helper_process_ids() -> HashSet<u32> {
    let mut ids = HashSet::new();
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return ids;
    }
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut has_entry = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    while has_entry {
        let name_len = entry
            .szExeFile
            .iter()
            .position(|character| *character == 0)
            .unwrap_or(entry.szExeFile.len());
        let name = String::from_utf16_lossy(&entry.szExeFile[..name_len]).to_ascii_lowercase();
        if matches!(
            name.as_str(),
            "samsunginternet.exe" | "samsung.browser.broker.exe"
        ) {
            ids.insert(entry.th32ProcessID);
        }
        has_entry = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
    }
    unsafe {
        CloseHandle(snapshot);
    }
    ids
}

#[cfg(not(target_os = "windows"))]
fn helper_process_ids() -> HashSet<u32> {
    HashSet::new()
}

#[cfg(target_os = "windows")]
fn terminate_new_helper_processes(baseline: &HashSet<u32>) {
    for pid in helper_process_ids().difference(baseline) {
        let process = unsafe { OpenProcess(PROCESS_TERMINATE, 0, *pid) };
        if process.is_null() {
            continue;
        }
        unsafe {
            TerminateProcess(process, 0);
            CloseHandle(process);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn terminate_new_helper_processes(_baseline: &HashSet<u32>) {}

#[cfg(not(target_os = "windows"))]
fn hide_process_windows(_pid: u32) {}

async fn close_browser(endpoint: &str) -> Result<(), String> {
    let version: Value = reqwest::Client::new()
        .get(format!("{endpoint}/json/version"))
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .map_err(|error| format!("Could not reach the hidden browser for shutdown: {error}"))?
        .json()
        .await
        .map_err(|error| format!("Could not read the hidden browser endpoint: {error}"))?;
    let websocket_url = version
        .get("webSocketDebuggerUrl")
        .and_then(Value::as_str)
        .ok_or_else(|| "The hidden browser did not expose its shutdown channel.".to_string())?;
    let (mut socket, _) = connect_async(websocket_url)
        .await
        .map_err(|error| format!("Could not open the hidden browser shutdown channel: {error}"))?;
    socket
        .send(Message::Text(
            json!({ "id": 99, "method": "Browser.close" })
                .to_string()
                .into(),
        ))
        .await
        .map_err(|error| format!("Could not close the hidden browser: {error}"))?;
    let _ = socket.close(None).await;
    Ok(())
}

pub async fn run_managed_operation(
    operation: &str,
    args: Value,
    endpoint: &str,
) -> Result<Value, String> {
    if !matches!(
        operation,
        "probe"
            | "list"
            | "list_categories"
            | "create_category"
            | "update_category"
            | "delete_category"
            | "get"
            | "create"
            | "update"
            | "delete"
    ) {
        return Err(format!("Unsupported Reminder operation: {operation}"));
    }
    let endpoint = normalize_endpoint(endpoint)?;
    let _guard = MANAGED_BROWSER_LOCK.lock().await;
    let cached_credential = { CREDENTIAL_CACHE.read().await.clone() };
    let mut credential = match cached_credential {
        Some(credential) => credential,
        None => {
            let credential = match load_persistent_credential()? {
                Some(credential) => credential,
                None => {
                    let credential = bootstrap_credential(&endpoint).await?;
                    store_persistent_credential(&credential)?;
                    credential
                }
            };
            *CREDENTIAL_CACHE.write().await = Some(credential.clone());
            credential
        }
    };

    if operation == "probe" && credential.needs_identity_refresh() {
        if let Ok(refreshed) = bootstrap_credential(&endpoint).await {
            store_persistent_credential(&refreshed)?;
            *CREDENTIAL_CACHE.write().await = Some(refreshed.clone());
            credential = refreshed;
        }
    }

    match cloud::run_operation(credential, operation, args.clone()).await {
        Ok(result) => Ok(result),
        Err(CloudError::Unauthorized) => {
            *CREDENTIAL_CACHE.write().await = None;
            delete_persistent_credential()?;
            let credential = bootstrap_credential(&endpoint).await?;
            store_persistent_credential(&credential)?;
            *CREDENTIAL_CACHE.write().await = Some(credential.clone());
            cloud::run_operation(credential, operation, args)
                .await
                .map_err(CloudError::message)
        }
        Err(error) => Err(error.message()),
    }
}

async fn bootstrap_credential(endpoint: &str) -> Result<CloudCredential, String> {
    if cdp_available(endpoint).await {
        let credential = acquire_credential(endpoint).await?;
        if endpoint == DEFAULT_ENDPOINT {
            close_browser(endpoint).await?;
            for _ in 0..20 {
                if !cdp_available(endpoint).await {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
        }
        return Ok(credential);
    }

    let browser = r"C:\Program Files\Samsung\Internet\Application\samsunginternet.exe";
    if !std::path::Path::new(browser).exists() {
        return Err(
            "SAMSUNG_BROWSER_NOT_FOUND: Samsung Browser for Windows is not installed in its standard location. Install it, open it once, and retry."
                .into(),
        );
    }
    let profile = trusted_profile()?;
    let port = endpoint_port(endpoint);
    let baseline_processes = helper_process_ids();
    let mut child = Command::new(browser)
        .args([
            "--remote-debugging-address=127.0.0.1".to_string(),
            format!("--remote-debugging-port={port}"),
            format!("--remote-allow-origins={endpoint}"),
            format!("--user-data-dir={}", profile.display()),
            "--window-position=-32000,-32000".to_string(),
            "--window-size=1,1".to_string(),
            "--disable-session-crashed-bubble".to_string(),
            "--no-first-run".to_string(),
            "--no-default-browser-check".to_string(),
            "about:blank".to_string(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Could not start the hidden Samsung Browser helper: {error}"))?;

    let mut ready = false;
    for _ in 0..40 {
        hide_process_windows(child.id());
        tokio::time::sleep(Duration::from_millis(250)).await;
        if cdp_available(endpoint).await {
            ready = true;
            break;
        }
        if child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_some()
        {
            break;
        }
    }

    if !ready {
        let _ = child.kill();
        tokio::time::sleep(Duration::from_millis(200)).await;
        terminate_new_helper_processes(&baseline_processes);
        return Err(
            "SAMSUNG_BROWSER_BUSY: Samsung Browser could not start its hidden sync helper. Close Samsung Browser completely, then retry."
                .into(),
        );
    }

    hide_process_windows(child.id());

    let result = acquire_credential(endpoint).await;
    let close_result = close_browser(endpoint).await;
    for _ in 0..20 {
        if !cdp_available(endpoint).await {
            break;
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    if cdp_available(endpoint).await {
        let _ = child.kill();
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    terminate_new_helper_processes(&baseline_processes);

    match (result, close_result) {
        (Ok(credential), Ok(())) => Ok(credential),
        (Ok(_), Err(error)) => Err(error),
        (Err(error), _) => Err(error),
    }
}

async fn list_targets(client: &reqwest::Client, endpoint: &str) -> Result<Vec<CdpTarget>, String> {
    client
        .get(format!("{endpoint}/json/list"))
        .send()
        .await
        .map_err(|error| format!("Could not connect to Samsung Browser: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Samsung Browser CDP rejected the request: {error}"))?
        .json::<Vec<CdpTarget>>()
        .await
        .map_err(|error| format!("Samsung Browser returned invalid target data: {error}"))
}

async fn ensure_calendar_target(endpoint: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|error| error.to_string())?;
    let find_target = |targets: Vec<CdpTarget>| {
        targets.into_iter().find_map(|target| {
            (target.target_type == "page" && target.url == CALENDAR_PAGE)
                .then_some(target.websocket_debugger_url)
                .flatten()
        })
    };

    if let Some(websocket_url) = find_target(list_targets(&client, endpoint).await?) {
        return Ok(websocket_url);
    }

    client
        .put(format!(
            "{endpoint}/json/new?{}",
            urlencoding::encode(CALENDAR_PAGE)
        ))
        .send()
        .await
        .map_err(|error| format!("Could not open Samsung Calendar: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Samsung Calendar target was rejected: {error}"))?;

    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        if let Some(websocket_url) = find_target(list_targets(&client, endpoint).await?) {
            return Ok(websocket_url);
        }
    }

    Err(format!(
        "Samsung's signed Calendar extension ({CALENDAR_EXTENSION_ID}) did not become available."
    ))
}

async fn evaluate(endpoint: &str, expression: String) -> Result<Value, String> {
    let websocket_url = ensure_calendar_target(endpoint).await?;
    let (mut socket, _) = connect_async(&websocket_url)
        .await
        .map_err(|error| format!("Could not open Samsung Browser's secure page: {error}"))?;
    let request_id = 1_u64;
    let request = json!({
        "id": request_id,
        "method": "Runtime.evaluate",
        "params": {
            "expression": expression,
            "awaitPromise": true,
            "returnByValue": true,
            "userGesture": true
        }
    });
    socket
        .send(Message::Text(request.to_string().into()))
        .await
        .map_err(|error| format!("Could not send the Reminder operation: {error}"))?;

    while let Some(message) = socket.next().await {
        let message =
            message.map_err(|error| format!("Samsung Browser connection closed: {error}"))?;
        let Message::Text(text) = message else {
            continue;
        };
        let response: Value = serde_json::from_str(&text)
            .map_err(|error| format!("Samsung Browser returned invalid JSON: {error}"))?;
        if response.get("id").and_then(Value::as_u64) != Some(request_id) {
            continue;
        }

        if let Some(error) = response.get("error") {
            return Err(error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Samsung Browser evaluation failed")
                .to_string());
        }

        if let Some(exception) = response.pointer("/result/exceptionDetails") {
            let description = exception
                .pointer("/exception/description")
                .and_then(Value::as_str)
                .or_else(|| exception.get("text").and_then(Value::as_str))
                .unwrap_or("Samsung Reminder operation failed");
            return Err(description
                .lines()
                .next()
                .unwrap_or(description)
                .trim_start_matches("Error: ")
                .to_string());
        }

        return response
            .pointer("/result/result/value")
            .cloned()
            .ok_or_else(|| "Samsung Browser returned no Reminder data.".to_string());
    }

    Err("Samsung Browser closed the Reminder operation unexpectedly.".into())
}

async fn acquire_credential(endpoint: &str) -> Result<CloudCredential, String> {
    let expression = format!(
        "({EXTENSION_RUNTIME})({},{})",
        serde_json::to_string("credential").map_err(|error| error.to_string())?,
        serde_json::to_string(&json!({})).map_err(|error| error.to_string())?
    );
    let mut credential_not_ready = false;
    for _ in 0..10 {
        match evaluate(endpoint, expression.clone()).await {
            Ok(value) => {
                return serde_json::from_value(value).map_err(|error| {
                    format!("Samsung returned an invalid cloud credential: {error}")
                });
            }
            Err(error)
                if error
                    .to_ascii_lowercase()
                    .contains("credential unavailable")
                    || error.contains("has not initialized") =>
            {
                credential_not_ready = true;
                tokio::time::sleep(Duration::from_millis(350)).await;
            }
            Err(error) => return Err(error),
        }
    }
    if credential_not_ready {
        Err(
            "SAMSUNG_ACCOUNT_NOT_SIGNED_IN: Sign in to your Samsung account in Samsung Browser, then retry."
                .into(),
        )
    } else {
        Err("Samsung Account did not become ready.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_endpoint;

    #[test]
    fn accepts_and_canonicalizes_loopback_cdp_endpoint() {
        assert_eq!(
            normalize_endpoint("http://127.0.0.1:9226/").as_deref(),
            Ok("http://127.0.0.1:9226")
        );
    }

    #[test]
    fn rejects_non_loopback_or_ambiguous_cdp_endpoints() {
        for endpoint in [
            "https://127.0.0.1:9226",
            "http://localhost:9226",
            "http://127.0.0.1:9226/json/version",
            "http://127.0.0.1:9226@public.example",
            "http://public.example:9226",
            "http://127.0.0.1",
        ] {
            assert!(normalize_endpoint(endpoint).is_err(), "accepted {endpoint}");
        }
    }
}
