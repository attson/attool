use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration as StdDuration,
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, State};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{tcp::OwnedWriteHalf, TcpStream},
    sync::{broadcast, oneshot, Mutex as AsyncMutex},
    time::{sleep, timeout, Duration, Instant},
};
use uuid::Uuid;

use super::install::{InstalledPlugin, PluginStore};

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const READINESS_POLL_INTERVAL: Duration = Duration::from_millis(25);

trait HttpServiceEventSink: Send + Sync {
    fn emit(&self, app_id: &str, event: Value);
}

struct TauriEventSink(AppHandle);

impl HttpServiceEventSink for TauriEventSink {
    fn emit(&self, app_id: &str, event: Value) {
        let _ = self.0.emit_to(
            "main",
            "plugin://http-service-event",
            json!({ "appId": app_id, "event": event }),
        );
    }
}

pub struct HttpServiceRuntime {
    store: Arc<Mutex<PluginStore>>,
    data_directory: PathBuf,
    event_sink: Arc<dyn HttpServiceEventSink>,
    clients: Arc<Mutex<HashMap<String, ConnectedService>>>,
    connect_locks: Arc<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>>,
}

struct ConnectedService {
    version: String,
    client: ServiceClient,
    process: Option<Box<dyn ServiceProcess>>,
}

impl HttpServiceRuntime {
    pub fn new(store: PluginStore, data_directory: PathBuf, app: AppHandle) -> Self {
        Self::with_sink(store, data_directory, Arc::new(TauriEventSink(app)))
    }

    #[cfg(test)]
    fn with_event_sink(
        store: PluginStore,
        data_directory: PathBuf,
        event_sink: Arc<dyn HttpServiceEventSink>,
    ) -> Self {
        Self::with_sink(store, data_directory, event_sink)
    }

    fn with_sink(
        store: PluginStore,
        data_directory: PathBuf,
        event_sink: Arc<dyn HttpServiceEventSink>,
    ) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
            data_directory,
            event_sink,
            clients: Arc::new(Mutex::new(HashMap::new())),
            connect_locks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn connect(&self, app_id: &str) -> Result<(), HttpServiceError> {
        let connect_lock = {
            let mut locks = self.connect_locks.lock().map_err(|_| {
                HttpServiceError::new(
                    "HTTP_SERVICE_STATE_LOCK_FAILED",
                    "HTTP service state lock failed",
                )
            })?;
            Arc::clone(
                locks
                    .entry(app_id.to_string())
                    .or_insert_with(|| Arc::new(AsyncMutex::new(()))),
            )
        };
        let _connect_guard = connect_lock.lock().await;
        let plugin = self.installed_plugin(app_id)?;
        let service = plugin.native_service.as_ref().ok_or_else(|| {
            HttpServiceError::new(
                "HTTP_SERVICE_NOT_DECLARED",
                "Plugin does not declare a native HTTP service",
            )
        })?;
        if service.protocol_version != 1 {
            return Err(HttpServiceError::new(
                "HTTP_SERVICE_PROTOCOL_UNSUPPORTED",
                format!(
                    "HTTP service protocol {} is not supported",
                    service.protocol_version
                ),
            ));
        }
        let executable = plugin.root.join(&service.entry);
        let bytes = std::fs::read(&executable).map_err(|error| {
            HttpServiceError::new(
                "HTTP_SERVICE_BINARY_READ_FAILED",
                format!("Cannot read installed HTTP service: {error}"),
            )
        })?;
        if hex::encode(Sha256::digest(&bytes)) != service.sha256 {
            return Err(HttpServiceError::new(
                "HTTP_SERVICE_BINARY_HASH_MISMATCH",
                "Installed HTTP service does not match the signed manifest",
            ));
        }
        if self
            .clients
            .lock()
            .map_err(|_| {
                HttpServiceError::new(
                    "HTTP_SERVICE_STATE_LOCK_FAILED",
                    "HTTP service state lock failed",
                )
            })?
            .get(app_id)
            .is_some_and(|connected| {
                connected.version == plugin.version && !connected.client.is_disconnected()
            })
        {
            return Ok(());
        }

        {
            let mut clients = self.clients.lock().map_err(|_| {
                HttpServiceError::new(
                    "HTTP_SERVICE_STATE_LOCK_FAILED",
                    "HTTP service state lock failed",
                )
            })?;
            let owned_previous = clients
                .get_mut(app_id)
                .filter(|connected| connected.version != plugin.version)
                .and_then(|connected| connected.process.as_mut());
            if let Some(process) = owned_previous {
                process.terminate().map_err(|error| {
                    HttpServiceError::new(
                        "HTTP_SERVICE_STOP_FAILED",
                        format!("Cannot stop previous HTTP service version: {error}"),
                    )
                })?;
                clients.remove(app_id);
            }
        }

        let mut supervisor = ServiceSupervisor::new(
            self.data_directory.join("service.endpoint.json"),
            self.data_directory.clone(),
            executable,
            format!("attool-{app_id}-{}", Uuid::new_v4()),
            service.version.clone(),
            StdDuration::from_secs(10),
        );
        let (client, process) = supervisor.connect_managed().await?;
        let mut events = client.subscribe();
        let event_sink = Arc::clone(&self.event_sink);
        let event_app_id = app_id.to_string();
        tokio::spawn(async move {
            loop {
                match events.recv().await {
                    Ok(event) => event_sink.emit(&event_app_id, event),
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        self.clients
            .lock()
            .map_err(|_| {
                HttpServiceError::new(
                    "HTTP_SERVICE_STATE_LOCK_FAILED",
                    "HTTP service state lock failed",
                )
            })?
            .insert(
                app_id.to_string(),
                ConnectedService {
                    version: plugin.version,
                    client,
                    process,
                },
            );
        Ok(())
    }

    pub async fn request(
        &self,
        app_id: &str,
        method: &str,
        params: Value,
    ) -> Result<Value, HttpServiceError> {
        let plugin = self.installed_plugin(app_id)?;
        let permission = required_permission(method)?;
        if !plugin
            .permissions
            .iter()
            .any(|declared| declared == permission)
        {
            return Err(HttpServiceError::new(
                "HTTP_SERVICE_PERMISSION_DENIED",
                format!("Plugin does not declare {permission}"),
            ));
        }
        let client = self
            .clients
            .lock()
            .map_err(|_| {
                HttpServiceError::new(
                    "HTTP_SERVICE_STATE_LOCK_FAILED",
                    "HTTP service state lock failed",
                )
            })?
            .get(app_id)
            .filter(|connected| connected.version == plugin.version)
            .map(|connected| connected.client.clone())
            .ok_or_else(|| {
                HttpServiceError::new(
                    "HTTP_SERVICE_NOT_CONNECTED",
                    "HTTP service is not connected for this plugin version",
                )
            })?;
        client.request(method, params).await
    }

    fn installed_plugin(&self, app_id: &str) -> Result<InstalledPlugin, HttpServiceError> {
        let plugin = self
            .store
            .lock()
            .map_err(|_| {
                HttpServiceError::new("HTTP_SERVICE_STORE_LOCK_FAILED", "Plugin store lock failed")
            })?
            .list()
            .map_err(|error| HttpServiceError::new(error.code, error.message))?
            .into_iter()
            .find(|plugin| plugin.app_id == app_id)
            .ok_or_else(|| {
                HttpServiceError::new(
                    "HTTP_SERVICE_PLUGIN_NOT_FOUND",
                    "Installed plugin was not found",
                )
            })?;
        if !plugin.enabled {
            return Err(HttpServiceError::new(
                "HTTP_SERVICE_PLUGIN_DISABLED",
                "Installed plugin is disabled",
            ));
        }
        Ok(plugin)
    }
}

#[tauri::command]
pub async fn plugin_connect_http_service(
    app_id: String,
    runtime: State<'_, HttpServiceRuntime>,
) -> Result<(), HttpServiceError> {
    runtime.connect(&app_id).await
}

#[tauri::command]
pub async fn plugin_http_service_request(
    app_id: String,
    method: String,
    params: Value,
    runtime: State<'_, HttpServiceRuntime>,
) -> Result<Value, HttpServiceError> {
    runtime.request(&app_id, &method, params).await
}

#[derive(Debug, Clone)]
pub struct ServiceLaunchSpec {
    pub endpoint_path: PathBuf,
    pub data_directory: PathBuf,
    pub service_version: String,
}

pub trait ServiceProcess: Send {
    fn try_exit(&mut self) -> io::Result<Option<i32>>;

    fn terminate(&mut self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "service process is not owned by this host",
        ))
    }
}

pub trait ServiceLauncher: Send + Sync {
    fn launch(&self, spec: &ServiceLaunchSpec)
        -> Result<Box<dyn ServiceProcess>, HttpServiceError>;
}

struct CommandServiceLauncher {
    executable: PathBuf,
}

impl CommandServiceLauncher {
    fn new(executable: PathBuf) -> Self {
        Self { executable }
    }
}

impl ServiceLauncher for CommandServiceLauncher {
    fn launch(
        &self,
        spec: &ServiceLaunchSpec,
    ) -> Result<Box<dyn ServiceProcess>, HttpServiceError> {
        let child = Command::new(&self.executable)
            .arg("--endpoint")
            .arg(&spec.endpoint_path)
            .arg("--data-directory")
            .arg(&spec.data_directory)
            .stdin(Stdio::null())
            .spawn()
            .map_err(|error| {
                HttpServiceError::new(
                    "HTTP_SERVICE_START_FAILED",
                    format!("Cannot start HTTP service: {error}"),
                )
            })?;
        Ok(Box::new(ChildProcess(child)))
    }
}

struct ChildProcess(Child);

impl ServiceProcess for ChildProcess {
    fn try_exit(&mut self) -> io::Result<Option<i32>> {
        self.0
            .try_wait()
            .map(|status| status.map(|status| status.code().unwrap_or(-1)))
    }

    fn terminate(&mut self) -> io::Result<()> {
        if self.0.try_wait()?.is_none() {
            self.0.kill()?;
            self.0.wait()?;
        }
        Ok(())
    }
}

struct ServiceSupervisor {
    endpoint_path: PathBuf,
    data_directory: PathBuf,
    client_id: String,
    service_version: String,
    startup_timeout: StdDuration,
    launcher: Arc<dyn ServiceLauncher>,
    process: Option<Box<dyn ServiceProcess>>,
}

impl ServiceSupervisor {
    fn new(
        endpoint_path: PathBuf,
        data_directory: PathBuf,
        executable: PathBuf,
        client_id: impl Into<String>,
        service_version: impl Into<String>,
        startup_timeout: StdDuration,
    ) -> Self {
        Self::with_launcher(
            endpoint_path,
            data_directory,
            client_id,
            service_version,
            startup_timeout,
            Arc::new(CommandServiceLauncher::new(executable)),
        )
    }

    fn with_launcher(
        endpoint_path: PathBuf,
        data_directory: PathBuf,
        client_id: impl Into<String>,
        service_version: impl Into<String>,
        startup_timeout: StdDuration,
        launcher: Arc<dyn ServiceLauncher>,
    ) -> Self {
        Self {
            endpoint_path,
            data_directory,
            client_id: client_id.into(),
            service_version: service_version.into(),
            startup_timeout,
            launcher,
            process: None,
        }
    }

    async fn connect(&mut self) -> Result<ServiceClient, HttpServiceError> {
        if self.endpoint_path.exists() {
            match ServiceClient::connect(&self.endpoint_path, &self.data_directory, &self.client_id)
                .await
            {
                Ok(client) => {
                    self.ensure_endpoint_version()?;
                    return Ok(client);
                }
                Err(error) if error.code == "HTTP_SERVICE_NOT_RUNNING" => {
                    remove_stale_endpoint(&self.endpoint_path, &self.data_directory)?;
                }
                Err(error) => return Err(error),
            }
        }

        let spec = ServiceLaunchSpec {
            endpoint_path: self.endpoint_path.clone(),
            data_directory: self.data_directory.clone(),
            service_version: self.service_version.clone(),
        };
        self.process = Some(self.launcher.launch(&spec)?);
        self.wait_until_ready().await
    }

    async fn connect_managed(
        &mut self,
    ) -> Result<(ServiceClient, Option<Box<dyn ServiceProcess>>), HttpServiceError> {
        let client = self.connect().await?;
        Ok((client, self.process.take()))
    }

    async fn wait_until_ready(&mut self) -> Result<ServiceClient, HttpServiceError> {
        let deadline = Instant::now() + self.startup_timeout;
        loop {
            if self.endpoint_path.exists() {
                match ServiceClient::connect(
                    &self.endpoint_path,
                    &self.data_directory,
                    &self.client_id,
                )
                .await
                {
                    Ok(client) => {
                        self.ensure_endpoint_version()?;
                        return Ok(client);
                    }
                    Err(error) if error.code == "HTTP_SERVICE_NOT_RUNNING" => {}
                    Err(error) => return Err(error),
                }
            }
            if let Some(exit_code) = self.process_exit_code()? {
                if self.data_directory.join("service.lock").exists() {
                    self.process = None;
                } else {
                    return Err(HttpServiceError::new(
                        "HTTP_SERVICE_START_FAILED",
                        format!("HTTP service exited before it became ready (status {exit_code})"),
                    ));
                }
            }
            if Instant::now() >= deadline {
                return Err(HttpServiceError::new(
                    "HTTP_SERVICE_START_TIMEOUT",
                    "HTTP service did not become ready before the startup timeout",
                ));
            }
            sleep(READINESS_POLL_INTERVAL).await;
        }
    }

    fn ensure_endpoint_version(&self) -> Result<(), HttpServiceError> {
        let endpoint = read_endpoint(&self.endpoint_path)?;
        if endpoint.service_version != self.service_version {
            return Err(HttpServiceError::new(
                "HTTP_SERVICE_VERSION_UNSUPPORTED",
                format!(
                    "HTTP service version {} does not match installed version {}",
                    endpoint.service_version, self.service_version
                ),
            ));
        }
        Ok(())
    }

    fn process_exit_code(&mut self) -> Result<Option<i32>, HttpServiceError> {
        self.process
            .as_mut()
            .map(|process| process.try_exit())
            .transpose()
            .map(|status| status.flatten())
            .map_err(|error| {
                HttpServiceError::new(
                    "HTTP_SERVICE_START_FAILED",
                    format!("Cannot inspect HTTP service process: {error}"),
                )
            })
    }
}

fn remove_stale_endpoint(path: &Path, data_directory: &Path) -> Result<(), HttpServiceError> {
    let endpoint = read_endpoint(path)?;
    std::fs::remove_file(path).map_err(|error| {
        HttpServiceError::new(
            "HTTP_SERVICE_ENDPOINT_WRITE_FAILED",
            format!("Cannot remove stale HTTP service endpoint: {error}"),
        )
    })?;
    let lease_path = data_directory.join("service.lock");
    if let Ok(lease) = std::fs::read_to_string(&lease_path) {
        if lease.lines().next() == Some(endpoint.token.as_str()) {
            std::fs::remove_file(lease_path).map_err(|error| {
                HttpServiceError::new(
                    "HTTP_SERVICE_LEASE_FAILED",
                    format!("Cannot remove stale HTTP service lease: {error}"),
                )
            })?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpServiceError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl HttpServiceError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: false,
        }
    }
}

impl std::fmt::Display for HttpServiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for HttpServiceError {}

fn required_permission(method: &str) -> Result<&'static str, HttpServiceError> {
    match method {
        "http.send" | "http.cancel" => Ok("host.http.request"),
        "stream.open" | "stream.close" | "stream.send" | "stream.snapshot" => {
            Ok("host.http.stream")
        }
        "data.tabs.list"
        | "data.tabs.upsert"
        | "data.tabs.delete"
        | "data.tabs.setActive"
        | "data.history.list"
        | "data.history.insert"
        | "data.history.delete"
        | "data.history.clear"
        | "data.envs.list"
        | "data.envs.upsert"
        | "data.envs.delete"
        | "data.envs.setActive"
        | "data.envs.vars.list"
        | "data.envs.vars.upsert"
        | "data.envs.vars.delete"
        | "data.collections.list"
        | "data.collections.folders.list"
        | "data.collections.requests.list"
        | "data.collections.upsert"
        | "data.collections.folders.upsert"
        | "data.collections.requests.upsert"
        | "data.collections.delete"
        | "data.collections.folders.delete"
        | "data.collections.requests.delete"
        | "data.openapi.import" => Ok("host.http.data"),
        _ => Err(HttpServiceError::new(
            "HTTP_SERVICE_METHOD_NOT_FOUND",
            "HTTP service method is not exposed by the host",
        )),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Endpoint {
    port: u16,
    token: String,
    service_version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorEnvelope {
    code: String,
    message: String,
    retryable: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
enum ServiceReply {
    #[serde(rename = "handshake")]
    Handshake {
        #[serde(rename = "requestId")]
        request_id: String,
    },
    #[serde(rename = "response")]
    Response {
        #[serde(rename = "requestId")]
        request_id: String,
        result: Value,
    },
    #[serde(rename = "error")]
    Error {
        #[serde(rename = "requestId")]
        request_id: String,
        error: ErrorEnvelope,
    },
    #[serde(rename = "event")]
    Event {
        #[serde(rename = "sessionId")]
        session_id: String,
        generation: u64,
        message: Value,
    },
}

type PendingResult = Result<Value, HttpServiceError>;

#[derive(Default)]
struct ClientState {
    pending: HashMap<String, oneshot::Sender<PendingResult>>,
    disconnected: Option<HttpServiceError>,
}

type SharedClientState = Arc<Mutex<ClientState>>;

#[derive(Clone)]
struct ServiceClient {
    writer: Arc<AsyncMutex<OwnedWriteHalf>>,
    state: SharedClientState,
    events: broadcast::Sender<Value>,
}

impl ServiceClient {
    async fn connect(
        endpoint_path: &Path,
        data_directory: &Path,
        client_id: &str,
    ) -> Result<Self, HttpServiceError> {
        let endpoint = read_endpoint(endpoint_path)?;
        let stream = TcpStream::connect(("127.0.0.1", endpoint.port))
            .await
            .map_err(|error| {
                let code = if error.kind() == std::io::ErrorKind::ConnectionRefused {
                    "HTTP_SERVICE_NOT_RUNNING"
                } else {
                    "HTTP_SERVICE_CONNECT_FAILED"
                };
                HttpServiceError::new(code, format!("Cannot connect to HTTP service: {error}"))
            })?;
        let (reader, mut writer) = stream.into_split();
        let request_id = Uuid::new_v4().to_string();
        write_json_line(
            &mut writer,
            &json!({
                "kind": "request",
                "protocolVersion": 1,
                "appId": "com.attson.http",
                "clientId": client_id,
                "requestId": request_id,
                "hostApi": "2.0.0",
                "dataApi": "1.0.0",
                "serviceApi": "1.0.0",
                "dataDirectory": data_directory.to_string_lossy(),
                "token": endpoint.token
            }),
        )
        .await?;

        let mut reader = BufReader::new(reader);
        let mut line = Vec::new();
        let read = timeout(Duration::from_secs(10), reader.read_until(b'\n', &mut line))
            .await
            .map_err(|_| {
                HttpServiceError::new(
                    "HTTP_SERVICE_HANDSHAKE_TIMEOUT",
                    "HTTP service handshake timed out",
                )
            })?
            .map_err(|error| {
                HttpServiceError::new(
                    "HTTP_SERVICE_HANDSHAKE_FAILED",
                    format!("Cannot read HTTP service handshake: {error}"),
                )
            })?;
        if read == 0 {
            return Err(disconnected());
        }
        let reply = serde_json::from_slice::<ServiceReply>(&line).map_err(|error| {
            HttpServiceError::new(
                "HTTP_SERVICE_HANDSHAKE_FAILED",
                format!("Cannot decode HTTP service handshake: {error}"),
            )
        })?;
        match reply {
            ServiceReply::Handshake {
                request_id: response_id,
            } if response_id == request_id => {}
            ServiceReply::Error { error, .. } => return Err(error.into()),
            _ => {
                return Err(HttpServiceError::new(
                    "HTTP_SERVICE_HANDSHAKE_FAILED",
                    "HTTP service returned an unexpected handshake response",
                ));
            }
        }

        let state = Arc::new(Mutex::new(ClientState::default()));
        let (events, _) = broadcast::channel(256);
        tokio::spawn(read_replies(reader, Arc::clone(&state), events.clone()));
        Ok(Self {
            writer: Arc::new(AsyncMutex::new(writer)),
            state,
            events,
        })
    }

    fn subscribe(&self) -> broadcast::Receiver<Value> {
        self.events.subscribe()
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, HttpServiceError> {
        let request_timeout = request_timeout(method, &params);
        let request_id = Uuid::new_v4().to_string();
        let request = json!({
            "kind": "request",
            "requestId": request_id,
            "method": method,
            "params": params
        });
        let (sender, receiver) = oneshot::channel();
        {
            let mut state = self.state.lock().expect("HTTP client state mutex poisoned");
            if let Some(error) = &state.disconnected {
                return Err(error.clone());
            }
            state.pending.insert(request_id.clone(), sender);
        }
        let _guard = PendingGuard {
            request_id,
            state: Arc::clone(&self.state),
        };
        write_json_line(&mut *self.writer.lock().await, &request).await?;
        match timeout(request_timeout, receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(disconnected()),
            Err(_) => Err(HttpServiceError::new(
                "HTTP_SERVICE_REQUEST_TIMEOUT",
                "HTTP service request timed out",
            )),
        }
    }

    fn is_disconnected(&self) -> bool {
        self.state
            .lock()
            .expect("HTTP client state mutex poisoned")
            .disconnected
            .is_some()
    }
}

fn request_timeout(method: &str, params: &Value) -> Duration {
    if method != "http.send" {
        return DEFAULT_REQUEST_TIMEOUT;
    }
    let seconds = params
        .get("request")
        .and_then(|request| request.get("timeoutSeconds"))
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_REQUEST_TIMEOUT.as_secs())
        .clamp(1, 300);
    Duration::from_secs(seconds + 5)
}

impl From<ErrorEnvelope> for HttpServiceError {
    fn from(error: ErrorEnvelope) -> Self {
        Self {
            code: error.code,
            message: error.message,
            retryable: error.retryable,
        }
    }
}

struct PendingGuard {
    request_id: String,
    state: SharedClientState,
}

impl Drop for PendingGuard {
    fn drop(&mut self) {
        self.state
            .lock()
            .expect("HTTP client state mutex poisoned")
            .pending
            .remove(&self.request_id);
    }
}

async fn read_replies(
    mut reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    state: SharedClientState,
    events: broadcast::Sender<Value>,
) {
    let mut line = Vec::new();
    let disconnect_error = loop {
        line.clear();
        match reader.read_until(b'\n', &mut line).await {
            Ok(0) => break disconnected(),
            Err(error) => {
                break HttpServiceError::new(
                    "HTTP_SERVICE_DISCONNECTED",
                    format!("Cannot read HTTP service response: {error}"),
                );
            }
            Ok(_) => {}
        }
        let reply = match serde_json::from_slice::<ServiceReply>(&line) {
            Ok(reply) => reply,
            Err(error) => {
                break HttpServiceError::new(
                    "HTTP_SERVICE_RESPONSE_INVALID",
                    format!("Cannot decode HTTP service response: {error}"),
                );
            }
        };
        match reply {
            ServiceReply::Response { request_id, result } => {
                if let Some(sender) = state
                    .lock()
                    .expect("HTTP client state mutex poisoned")
                    .pending
                    .remove(&request_id)
                {
                    let _ = sender.send(Ok(result));
                }
            }
            ServiceReply::Error { request_id, error } => {
                if let Some(sender) = state
                    .lock()
                    .expect("HTTP client state mutex poisoned")
                    .pending
                    .remove(&request_id)
                {
                    let _ = sender.send(Err(error.into()));
                }
            }
            ServiceReply::Event {
                session_id,
                generation,
                message,
            } => {
                let _ = events.send(json!({
                    "sessionId": session_id,
                    "generation": generation,
                    "message": message
                }));
            }
            ServiceReply::Handshake { .. } => {
                break HttpServiceError::new(
                    "HTTP_SERVICE_RESPONSE_INVALID",
                    "Unexpected handshake response after connection was established",
                );
            }
        }
    };
    let senders = {
        let mut state = state.lock().expect("HTTP client state mutex poisoned");
        state.disconnected = Some(disconnect_error.clone());
        state
            .pending
            .drain()
            .map(|(_, sender)| sender)
            .collect::<Vec<_>>()
    };
    for sender in senders {
        let _ = sender.send(Err(disconnect_error.clone()));
    }
}

async fn write_json_line(
    writer: &mut OwnedWriteHalf,
    value: &impl Serialize,
) -> Result<(), HttpServiceError> {
    let mut bytes = serde_json::to_vec(value).map_err(|error| {
        HttpServiceError::new(
            "HTTP_SERVICE_REQUEST_INVALID",
            format!("Cannot encode HTTP service request: {error}"),
        )
    })?;
    bytes.push(b'\n');
    writer.write_all(&bytes).await.map_err(|error| {
        HttpServiceError::new(
            "HTTP_SERVICE_DISCONNECTED",
            format!("Cannot write HTTP service request: {error}"),
        )
    })
}

fn read_endpoint(path: &Path) -> Result<Endpoint, HttpServiceError> {
    let bytes = std::fs::read(path).map_err(|error| {
        HttpServiceError::new(
            "HTTP_SERVICE_ENDPOINT_READ_FAILED",
            format!("Cannot read HTTP service endpoint: {error}"),
        )
    })?;
    let endpoint: Endpoint = serde_json::from_slice(&bytes).map_err(|error| {
        HttpServiceError::new(
            "HTTP_SERVICE_ENDPOINT_INVALID",
            format!("Cannot decode HTTP service endpoint: {error}"),
        )
    })?;
    if endpoint.port == 0
        || endpoint.token.trim().is_empty()
        || endpoint.service_version.trim().is_empty()
    {
        return Err(HttpServiceError::new(
            "HTTP_SERVICE_ENDPOINT_INVALID",
            "HTTP service endpoint fields are invalid",
        ));
    }
    Ok(endpoint)
}

fn disconnected() -> HttpServiceError {
    HttpServiceError::new(
        "HTTP_SERVICE_DISCONNECTED",
        "HTTP service connection was closed",
    )
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        io,
        path::Path,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
        time::Duration,
    };

    use ed25519_dalek::VerifyingKey;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use tempfile::tempdir;
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::TcpListener,
    };

    use super::{
        request_timeout, required_permission, HttpServiceError, HttpServiceEventSink,
        HttpServiceRuntime, ServiceClient, ServiceLaunchSpec, ServiceLauncher, ServiceProcess,
        ServiceSupervisor, DEFAULT_REQUEST_TIMEOUT,
    };
    use crate::plugin::{install::PluginStore, package::HostEnvironment};

    struct NoopEventSink;

    struct RecordingEventSink {
        sender: tokio::sync::mpsc::UnboundedSender<(String, serde_json::Value)>,
    }

    impl HttpServiceEventSink for NoopEventSink {
        fn emit(&self, _app_id: &str, _event: serde_json::Value) {}
    }

    impl HttpServiceEventSink for RecordingEventSink {
        fn emit(&self, app_id: &str, event: serde_json::Value) {
            let _ = self.sender.send((app_id.to_string(), event));
        }
    }

    struct PanicLauncher;

    struct TestLauncher {
        starts: Arc<AtomicUsize>,
        server: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    }

    struct TestProcess {
        server: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    }

    struct ExitedProcess;

    struct ExitingLauncher;

    struct LosingRaceLauncher {
        server: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    }

    struct MissingWinnerLauncher;

    struct OwnedTestLauncher {
        server: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
        stops: Arc<AtomicUsize>,
    }

    struct OwnedTestProcess {
        server: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
        stops: Arc<AtomicUsize>,
    }

    impl ServiceLauncher for PanicLauncher {
        fn launch(
            &self,
            _spec: &ServiceLaunchSpec,
        ) -> Result<Box<dyn ServiceProcess>, HttpServiceError> {
            panic!("healthy service must be reused")
        }
    }

    impl ServiceLauncher for TestLauncher {
        fn launch(
            &self,
            spec: &ServiceLaunchSpec,
        ) -> Result<Box<dyn ServiceProcess>, HttpServiceError> {
            self.starts.fetch_add(1, Ordering::SeqCst);
            let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
            listener.set_nonblocking(true).unwrap();
            let listener = TcpListener::from_std(listener).unwrap();
            std::fs::create_dir_all(spec.endpoint_path.parent().unwrap()).unwrap();
            std::fs::write(
                &spec.endpoint_path,
                serde_json::to_vec(&json!({
                    "port": listener.local_addr().unwrap().port(),
                    "token": "secret-token",
                    "serviceVersion": spec.service_version
                }))
                .unwrap(),
            )
            .unwrap();
            let data_directory = spec.data_directory.to_string_lossy().to_string();
            *self.server.lock().unwrap() =
                Some(tokio::spawn(fake_service(listener, data_directory)));
            Ok(Box::new(TestProcess {
                server: Arc::clone(&self.server),
            }))
        }
    }

    impl ServiceProcess for TestProcess {
        fn try_exit(&mut self) -> io::Result<Option<i32>> {
            Ok(self
                .server
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|task| task.is_finished())
                .then_some(0))
        }
    }

    impl ServiceProcess for ExitedProcess {
        fn try_exit(&mut self) -> io::Result<Option<i32>> {
            Ok(Some(17))
        }
    }

    impl ServiceLauncher for ExitingLauncher {
        fn launch(
            &self,
            _spec: &ServiceLaunchSpec,
        ) -> Result<Box<dyn ServiceProcess>, HttpServiceError> {
            Ok(Box::new(ExitedProcess))
        }
    }

    impl ServiceLauncher for LosingRaceLauncher {
        fn launch(
            &self,
            spec: &ServiceLaunchSpec,
        ) -> Result<Box<dyn ServiceProcess>, HttpServiceError> {
            std::fs::create_dir_all(&spec.data_directory).unwrap();
            std::fs::write(spec.data_directory.join("service.lock"), "winner-token").unwrap();
            let spec = spec.clone();
            let task = tokio::spawn(async move {
                tokio::task::yield_now().await;
                std::fs::remove_file(spec.data_directory.join("service.lock")).unwrap();
                let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
                std::fs::write(
                    &spec.endpoint_path,
                    serde_json::to_vec(&json!({
                        "port": listener.local_addr().unwrap().port(),
                        "token": "secret-token",
                        "serviceVersion": spec.service_version
                    }))
                    .unwrap(),
                )
                .unwrap();
                fake_service(listener, spec.data_directory.to_string_lossy().to_string()).await;
            });
            *self.server.lock().unwrap() = Some(task);
            Ok(Box::new(ExitedProcess))
        }
    }

    impl ServiceLauncher for MissingWinnerLauncher {
        fn launch(
            &self,
            spec: &ServiceLaunchSpec,
        ) -> Result<Box<dyn ServiceProcess>, HttpServiceError> {
            std::fs::create_dir_all(&spec.data_directory).unwrap();
            std::fs::write(spec.data_directory.join("service.lock"), "missing-winner").unwrap();
            Ok(Box::new(ExitedProcess))
        }
    }

    impl ServiceLauncher for OwnedTestLauncher {
        fn launch(
            &self,
            spec: &ServiceLaunchSpec,
        ) -> Result<Box<dyn ServiceProcess>, HttpServiceError> {
            let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
            listener.set_nonblocking(true).unwrap();
            let listener = TcpListener::from_std(listener).unwrap();
            std::fs::create_dir_all(spec.endpoint_path.parent().unwrap()).unwrap();
            std::fs::write(
                &spec.endpoint_path,
                serde_json::to_vec(&json!({
                    "port": listener.local_addr().unwrap().port(),
                    "token": "secret-token",
                    "serviceVersion": spec.service_version
                }))
                .unwrap(),
            )
            .unwrap();
            let data_directory = spec.data_directory.to_string_lossy().to_string();
            *self.server.lock().unwrap() =
                Some(tokio::spawn(fake_service(listener, data_directory)));
            Ok(Box::new(OwnedTestProcess {
                server: Arc::clone(&self.server),
                stops: Arc::clone(&self.stops),
            }))
        }
    }

    impl ServiceProcess for OwnedTestProcess {
        fn try_exit(&mut self) -> io::Result<Option<i32>> {
            Ok(None)
        }

        fn terminate(&mut self) -> io::Result<()> {
            if let Some(server) = self.server.lock().unwrap().take() {
                server.abort();
            }
            self.stops.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn maps_only_declared_http_service_methods_to_permissions() {
        assert_eq!(
            required_permission("http.send").unwrap(),
            "host.http.request"
        );
        assert_eq!(
            required_permission("http.cancel").unwrap(),
            "host.http.request"
        );
        assert_eq!(
            required_permission("stream.open").unwrap(),
            "host.http.stream"
        );
        assert_eq!(
            required_permission("stream.snapshot").unwrap(),
            "host.http.stream"
        );
        assert_eq!(
            required_permission("data.tabs.list").unwrap(),
            "host.http.data"
        );
        assert_eq!(
            required_permission("data.openapi.import").unwrap(),
            "host.http.data"
        );
        assert!(required_permission("http.data.tabs.list").is_err());
        assert!(required_permission("stream.unknown").is_err());
        assert!(required_permission("data.unknown.read").is_err());
    }

    #[test]
    fn http_request_timeout_respects_the_user_setting() {
        assert_eq!(
            request_timeout(
                "http.send",
                &json!({ "request": { "timeoutSeconds": 300 } })
            ),
            Duration::from_secs(305)
        );
        assert_eq!(
            request_timeout("data.tabs.list", &json!({})),
            DEFAULT_REQUEST_TIMEOUT
        );
    }

    #[tokio::test]
    async fn service_client_handshakes_and_routes_responses_and_events() {
        let directory = tempdir().unwrap();
        let data_directory = directory.path().join("http");
        std::fs::create_dir_all(&data_directory).unwrap();
        let endpoint_path = data_directory.join("service.endpoint.json");
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        std::fs::write(
            &endpoint_path,
            serde_json::to_vec(&json!({
                "port": listener.local_addr().unwrap().port(),
                "token": "secret-token",
                "serviceVersion": "0.1.0-alpha.0"
            }))
            .unwrap(),
        )
        .unwrap();
        let expected_data_directory = data_directory.to_string_lossy().to_string();
        let server = tokio::spawn(fake_service(listener, expected_data_directory.clone()));

        let client = ServiceClient::connect(
            &endpoint_path,
            Path::new(&expected_data_directory),
            "attool-test",
        )
        .await
        .unwrap();
        let mut events = client.subscribe();
        let result = client.request("data.tabs.list", json!({})).await.unwrap();

        assert_eq!(result, json!([{"id": "tab-1"}]));
        assert_eq!(
            events.recv().await.unwrap(),
            json!({
                "sessionId": "session-1",
                "generation": 3,
                "message": { "kind": "open", "atMs": 1, "status": 200, "headers": [] }
            })
        );
        server.await.unwrap();
    }

    #[tokio::test]
    async fn supervisor_reuses_a_healthy_service_with_the_installed_version() {
        let directory = tempdir().unwrap();
        let data_directory = directory.path().join("http");
        std::fs::create_dir_all(&data_directory).unwrap();
        let endpoint_path = data_directory.join("service.endpoint.json");
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        std::fs::write(
            &endpoint_path,
            serde_json::to_vec(&json!({
                "port": listener.local_addr().unwrap().port(),
                "token": "secret-token",
                "serviceVersion": "0.1.0-alpha.0"
            }))
            .unwrap(),
        )
        .unwrap();
        let expected_data_directory = data_directory.to_string_lossy().to_string();
        let server = tokio::spawn(fake_service(listener, expected_data_directory.clone()));
        let mut supervisor = ServiceSupervisor::with_launcher(
            endpoint_path,
            data_directory,
            "attool-test",
            "0.1.0-alpha.0",
            Duration::from_secs(1),
            Arc::new(PanicLauncher),
        );

        let client = supervisor.connect().await.unwrap();
        assert_eq!(
            client.request("data.tabs.list", json!({})).await.unwrap(),
            json!([{"id": "tab-1"}])
        );
        server.await.unwrap();
    }

    #[tokio::test]
    async fn supervisor_starts_the_installed_service_and_waits_until_ready() {
        let directory = tempdir().unwrap();
        let data_directory = directory.path().join("http");
        let endpoint_path = data_directory.join("service.endpoint.json");
        let starts = Arc::new(AtomicUsize::new(0));
        let server = Arc::new(Mutex::new(None));
        let mut supervisor = ServiceSupervisor::with_launcher(
            endpoint_path,
            data_directory,
            "attool-test",
            "0.1.0-alpha.0",
            Duration::from_secs(1),
            Arc::new(TestLauncher {
                starts: Arc::clone(&starts),
                server: Arc::clone(&server),
            }),
        );

        let client = supervisor.connect().await.unwrap();
        assert_eq!(starts.load(Ordering::SeqCst), 1);
        assert_eq!(
            client.request("data.tabs.list", json!({})).await.unwrap(),
            json!([{"id": "tab-1"}])
        );
        server.lock().unwrap().take().unwrap().await.unwrap();
    }

    #[tokio::test]
    async fn supervisor_returns_owned_process_for_version_takeover() {
        let directory = tempdir().unwrap();
        let data_directory = directory.path().join("http");
        let server = Arc::new(Mutex::new(None));
        let stops = Arc::new(AtomicUsize::new(0));
        let mut supervisor = ServiceSupervisor::with_launcher(
            data_directory.join("service.endpoint.json"),
            data_directory,
            "attool-test",
            "0.1.0-alpha.0",
            Duration::from_secs(1),
            Arc::new(OwnedTestLauncher {
                server: Arc::clone(&server),
                stops: Arc::clone(&stops),
            }),
        );

        let (_client, mut process) = supervisor.connect_managed().await.unwrap();
        process.as_mut().unwrap().terminate().unwrap();

        assert_eq!(stops.load(Ordering::SeqCst), 1);
        assert!(server.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn supervisor_replaces_a_dead_endpoint_from_an_older_service_version() {
        let directory = tempdir().unwrap();
        let data_directory = directory.path().join("http");
        let endpoint_path = data_directory.join("service.endpoint.json");
        std::fs::create_dir_all(&data_directory).unwrap();
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let dead_port = listener.local_addr().unwrap().port();
        drop(listener);
        std::fs::write(
            &endpoint_path,
            serde_json::to_vec(&json!({
                "port": dead_port,
                "token": "stale-token",
                "serviceVersion": "0.0.1"
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(data_directory.join("service.lock"), "stale-token\n123").unwrap();
        let starts = Arc::new(AtomicUsize::new(0));
        let server = Arc::new(Mutex::new(None));
        let mut supervisor = ServiceSupervisor::with_launcher(
            endpoint_path,
            data_directory,
            "attool-test",
            "0.1.0-alpha.0",
            Duration::from_secs(1),
            Arc::new(TestLauncher {
                starts: Arc::clone(&starts),
                server: Arc::clone(&server),
            }),
        );

        let client = supervisor.connect().await.unwrap();
        assert_eq!(starts.load(Ordering::SeqCst), 1);
        assert_eq!(
            client.request("data.tabs.list", json!({})).await.unwrap(),
            json!([{"id": "tab-1"}])
        );
        server.lock().unwrap().take().unwrap().await.unwrap();
    }

    #[tokio::test]
    async fn supervisor_reports_a_service_that_exits_before_readiness() {
        let directory = tempdir().unwrap();
        let data_directory = directory.path().join("http");
        let mut supervisor = ServiceSupervisor::with_launcher(
            data_directory.join("service.endpoint.json"),
            data_directory,
            "attool-test",
            "0.1.0-alpha.0",
            Duration::from_secs(1),
            Arc::new(ExitingLauncher),
        );

        let error = match supervisor.connect().await {
            Ok(_) => panic!("exited service must not produce a client"),
            Err(error) => error,
        };

        assert_eq!(error.code, "HTTP_SERVICE_START_FAILED");
        assert!(error.message.contains("17"));
    }

    #[tokio::test]
    async fn supervisor_startup_race_loser_waits_for_the_winning_service() {
        let directory = tempdir().unwrap();
        let data_directory = directory.path().join("http");
        let server = Arc::new(Mutex::new(None));
        let mut supervisor = ServiceSupervisor::with_launcher(
            data_directory.join("service.endpoint.json"),
            data_directory,
            "attool-test",
            "0.1.0-alpha.0",
            Duration::from_secs(1),
            Arc::new(LosingRaceLauncher {
                server: Arc::clone(&server),
            }),
        );

        let client = supervisor.connect().await.unwrap();
        assert_eq!(
            client.request("data.tabs.list", json!({})).await.unwrap(),
            json!([{"id": "tab-1"}])
        );
        server.lock().unwrap().take().unwrap().await.unwrap();
    }

    #[tokio::test]
    async fn supervisor_startup_race_without_a_winner_respects_the_timeout() {
        let directory = tempdir().unwrap();
        let data_directory = directory.path().join("http");
        let mut supervisor = ServiceSupervisor::with_launcher(
            data_directory.join("service.endpoint.json"),
            data_directory,
            "attool-test",
            "0.1.0-alpha.0",
            Duration::from_millis(50),
            Arc::new(MissingWinnerLauncher),
        );

        let error = match supervisor.connect().await {
            Ok(_) => panic!("missing winner must not produce a client"),
            Err(error) => error,
        };

        assert_eq!(error.code, "HTTP_SERVICE_START_TIMEOUT");
    }

    #[tokio::test]
    async fn runtime_rejects_a_tampered_native_service_before_connecting() {
        let directory = tempdir().unwrap();
        let store_root = directory.path().join("plugins");
        let store = native_service_store(&store_root, b"tampered-service", b"signed-service");
        let runtime = HttpServiceRuntime::with_event_sink(
            store,
            directory.path().join("http"),
            Arc::new(NoopEventSink),
        );

        let error = runtime.connect("com.attson.http").await.unwrap_err();

        assert_eq!(error.code, "HTTP_SERVICE_BINARY_HASH_MISMATCH");
        assert!(!directory.path().join("http/service.endpoint.json").exists());
    }

    #[tokio::test]
    async fn runtime_connects_requests_and_forwards_events_for_the_plugin() {
        let directory = tempdir().unwrap();
        let store_root = directory.path().join("plugins");
        let store = native_service_store(&store_root, b"signed-service", b"signed-service");
        let data_directory = directory.path().join("http");
        std::fs::create_dir_all(&data_directory).unwrap();
        let endpoint_path = data_directory.join("service.endpoint.json");
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        std::fs::write(
            &endpoint_path,
            serde_json::to_vec(&json!({
                "port": listener.local_addr().unwrap().port(),
                "token": "secret-token",
                "serviceVersion": "0.1.0-alpha.0"
            }))
            .unwrap(),
        )
        .unwrap();
        let expected_data_directory = data_directory.to_string_lossy().to_string();
        let server = tokio::spawn(fake_service(listener, expected_data_directory));
        let (event_sender, mut event_receiver) = tokio::sync::mpsc::unbounded_channel();
        let runtime = HttpServiceRuntime::with_event_sink(
            store,
            data_directory,
            Arc::new(RecordingEventSink {
                sender: event_sender,
            }),
        );

        runtime.connect("com.attson.http").await.unwrap();
        let result = runtime
            .request("com.attson.http", "data.tabs.list", json!({}))
            .await
            .unwrap();
        let (app_id, event) = event_receiver.recv().await.unwrap();

        assert_eq!(result, json!([{"id": "tab-1"}]));
        assert_eq!(app_id, "com.attson.http");
        assert_eq!(event["sessionId"], "session-1");
        server.await.unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    #[ignore = "requires ATHTTP_TEST_PACKAGE"]
    async fn packaged_plugin_installs_launches_and_shares_data_with_a_second_client() {
        let package =
            std::env::var("ATHTTP_TEST_PACKAGE").expect("ATHTTP_TEST_PACKAGE is required");
        let directory = tempdir().unwrap();
        let store = PluginStore::open(directory.path().join("plugins")).unwrap();
        let public_key = VerifyingKey::from_bytes(&[
            0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64,
            0x07, 0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68,
            0xf7, 0x07, 0x51, 0x1a,
        ])
        .unwrap();
        let keys = HashMap::from([("attool-p0-test".to_string(), public_key)]);
        let installed = store
            .install(
                Path::new(&package),
                &HostEnvironment::current("2.0.0"),
                &keys,
            )
            .unwrap();
        assert_eq!(installed.app_id, "com.attson.http");
        assert!(installed.native_service.is_some());

        let data_directory = directory.path().join("http");
        let runtime = HttpServiceRuntime::with_event_sink(
            store,
            data_directory.clone(),
            Arc::new(NoopEventSink),
        );
        runtime.connect("com.attson.http").await.unwrap();
        runtime
            .request(
                "com.attson.http",
                "data.tabs.upsert",
                json!({
                    "row": {
                        "id": "shared-tab",
                        "title": "Shared request",
                        "orderIndex": 0,
                        "isActive": true,
                        "specJson": "{}",
                        "updatedAt": 1_797_000_000_000_i64,
                        "kind": "http"
                    }
                }),
            )
            .await
            .unwrap();

        let second_client = ServiceClient::connect(
            &data_directory.join("service.endpoint.json"),
            &data_directory,
            "standalone-test",
        )
        .await
        .unwrap();
        let tabs = second_client
            .request("data.tabs.list", json!({}))
            .await
            .unwrap();
        assert_eq!(tabs[0]["id"], "shared-tab");

        let upstream = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let upstream_address = upstream.local_addr().unwrap();
        let upstream_task = tokio::spawn(async move {
            let (mut stream, _) = upstream.accept().await.unwrap();
            let mut request = Vec::new();
            BufReader::new(&mut stream)
                .read_until(b'\n', &mut request)
                .await
                .unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nhi")
                .await
                .unwrap();
        });
        let response = runtime
            .request(
                "com.attson.http",
                "http.send",
                json!({
                    "request": {
                        "method": "GET",
                        "url": format!("http://{upstream_address}/hello"),
                        "timeoutSeconds": 5
                    },
                    "cancelTokenId": "package-smoke"
                }),
            )
            .await
            .unwrap();
        assert_eq!(response["status"], 200);
        assert_eq!(response["body"], "hi");
        upstream_task.await.unwrap();

        stop_packaged_service(&data_directory);
    }

    #[cfg(unix)]
    fn stop_packaged_service(data_directory: &Path) {
        let lease = std::fs::read_to_string(data_directory.join("service.lock")).unwrap();
        let pid = lease.lines().nth(1).unwrap();
        let status = std::process::Command::new("kill")
            .args(["-TERM", pid])
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn native_service_store(
        store_root: &Path,
        service_bytes: &[u8],
        signed_bytes: &[u8],
    ) -> PluginStore {
        let store = PluginStore::open(store_root.to_path_buf()).unwrap();
        let version = "0.1.0-alpha.0";
        let plugin_root = store_root.join("apps/com.attson.http").join(version);
        let service_entry = "native/athttp-service";
        std::fs::create_dir_all(plugin_root.join("native")).unwrap();
        std::fs::write(plugin_root.join(service_entry), service_bytes).unwrap();
        std::fs::write(plugin_root.join("index.html"), b"<!doctype html>").unwrap();
        let service_hash = hex::encode(Sha256::digest(signed_bytes));
        let files = json!({
            "index.html": hex::encode(Sha256::digest(b"<!doctype html>")),
            service_entry: service_hash
        });
        let native_service = json!({
            "entry": service_entry,
            "version": version,
            "sha256": service_hash,
            "protocolVersion": 1
        });
        let connection = rusqlite::Connection::open(store_root.join("registry.sqlite3")).unwrap();
        connection
            .execute(
                "INSERT INTO installed_plugins (
                    app_id, name, version, publisher_id, publisher_name, publisher_key_id,
                    entry, permissions_json, files_json, native_service_json, enabled, installed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11)",
                rusqlite::params![
                    "com.attson.http",
                    "AT HTTP",
                    version,
                    "attson",
                    "Attson",
                    "attool-p0-test",
                    "index.html",
                    json!(["host.http.request", "host.http.stream", "host.http.data"]).to_string(),
                    files.to_string(),
                    native_service.to_string(),
                    "2026-11-13T00:00:00Z"
                ],
            )
            .unwrap();
        store
    }

    async fn fake_service(listener: TcpListener, data_directory: String) {
        let (stream, _) = listener.accept().await.unwrap();
        let (read, mut write) = stream.into_split();
        let mut reader = BufReader::new(read);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let handshake: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(handshake["kind"], "request");
        assert_eq!(handshake["token"], "secret-token");
        assert_eq!(handshake["dataDirectory"], data_directory);
        let handshake_id = handshake["requestId"].as_str().unwrap();
        write
            .write_all(
                format!(
                    "{}\n",
                    json!({
                        "kind": "handshake",
                        "protocolVersion": 1,
                        "requestId": handshake_id,
                        "appId": "com.attson.http",
                        "hostApi": "2.0.0",
                        "dataApi": "1.0.0",
                        "serviceApi": "1.0.0",
                        "schemaVersion": 1,
                        "migrationState": "ready",
                        "leaseState": "owner",
                        "capabilities": {
                            "httpRequest": true,
                            "httpStream": true,
                            "dataStore": true,
                            "openapiImport": true
                        }
                    })
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        line.clear();
        reader.read_line(&mut line).await.unwrap();
        let request: serde_json::Value = serde_json::from_str(&line).unwrap();
        let request_id = request["requestId"].as_str().unwrap();
        write
            .write_all(
                format!(
                    "{}\n{}\n",
                    json!({
                        "kind": "event",
                        "sessionId": "session-1",
                        "generation": 3,
                        "message": { "kind": "open", "atMs": 1, "status": 200, "headers": [] }
                    }),
                    json!({
                        "kind": "response",
                        "requestId": request_id,
                        "result": [{"id": "tab-1"}]
                    })
                )
                .as_bytes(),
            )
            .await
            .unwrap();
    }
}
