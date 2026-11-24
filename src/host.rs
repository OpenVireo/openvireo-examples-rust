//! The Host: everything the core asks the platform to do. The core does no I/O of its own, so every
//! program passes it HTTP, a secure store, an offline cache, a clock and a log sink.
//!
//! This one is small: reqwest with no redirects and no cookies, and memory for the store and the
//! cache. A real app keeps the store in the OS keychain and the cache on disk, so the device stays
//! trusted and vaults open offline after a restart.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ov_core::host::{
    BlobCache, ByteStream, CachedVault, Clock, Host, HttpRequest, HttpResponse, HttpTransport, Level, LogSink, Method,
    SecureStore, TransportError,
};
use ov_core::OvError;
use zeroize::Zeroizing;

/// The Host the examples use.
pub fn make_host() -> Host {
    Host {
        http: Arc::new(Http::new()),
        store: Arc::new(MemoryStore::default()),
        cache: Arc::new(MemoryCache::default()),
        clock: Arc::new(SystemClock),
        log: Arc::new(StderrLog),
    }
}

// ------------------------------------------------------------------------------------------ HTTP

/// HTTP with reqwest. Two rules for every transport: don't follow redirects, and don't send or
/// keep cookies (reqwest keeps none unless asked).
struct Http {
    client: reqwest::Client,
}

impl Http {
    fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(concat!("openvireo-examples-rust/", env!("CARGO_PKG_VERSION")))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("the HTTP client starts");
        Http { client }
    }

    fn build(&self, req: HttpRequest) -> reqwest::RequestBuilder {
        let method = match req.method {
            Method::Get => reqwest::Method::GET,
            Method::Head => reqwest::Method::HEAD,
            Method::Post => reqwest::Method::POST,
            Method::Put => reqwest::Method::PUT,
            Method::Patch => reqwest::Method::PATCH,
            Method::Delete => reqwest::Method::DELETE,
        };
        let mut b = self.client.request(method, &req.url);
        for (name, value) in &req.headers {
            b = b.header(name, value);
        }
        if let Some(body) = req.body {
            b = b.body(body);
        }
        b
    }
}

fn transport_error(e: reqwest::Error) -> TransportError {
    if e.is_timeout() {
        TransportError::Timeout
    } else if e.is_connect() {
        TransportError::Offline // the core then falls back to the offline cache
    } else {
        TransportError::Other(e.without_url().to_string())
    }
}

#[async_trait::async_trait]
impl HttpTransport for Http {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, TransportError> {
        let r = self.build(req).send().await.map_err(transport_error)?;
        let status = r.status().as_u16();
        let headers = r
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), String::from_utf8_lossy(v.as_bytes()).into_owned()))
            .collect();
        let body = r.bytes().await.map_err(transport_error)?.to_vec();
        Ok(HttpResponse { status, headers, body })
    }

    /// The events stream: a response body read chunk by chunk.
    async fn open_stream(&self, req: HttpRequest) -> Result<Box<dyn ByteStream>, TransportError> {
        let r = self.build(req).send().await.map_err(transport_error)?;
        Ok(Box::new(Stream { status: r.status().as_u16(), response: tokio::sync::Mutex::new(Some(r)) }))
    }
}

struct Stream {
    status: u16,
    response: tokio::sync::Mutex<Option<reqwest::Response>>,
}

#[async_trait::async_trait]
impl ByteStream for Stream {
    fn status(&self) -> u16 {
        self.status
    }

    async fn next_chunk(&self) -> Result<Option<Vec<u8>>, TransportError> {
        let mut guard = self.response.lock().await;
        let Some(r) = guard.as_mut() else { return Ok(None) };
        match r.chunk().await.map_err(transport_error)? {
            Some(chunk) => Ok(Some(chunk.to_vec())),
            None => {
                *guard = None;
                Ok(None)
            }
        }
    }

    fn close(&self) {
        // Dropping the response closes the connection. A read in progress holds the lock and ends
        // when the core stops polling it.
        if let Ok(mut guard) = self.response.try_lock() {
            *guard = None;
        }
    }
}

// ------------------------------------------------------------------------------ store and cache

/// Secrets that should persist: the session, the trusted Secret Key, self-hosted pairings. In
/// memory here; use the OS keychain in an app.
#[derive(Default)]
struct MemoryStore(Mutex<HashMap<String, Zeroizing<Vec<u8>>>>);

#[async_trait::async_trait]
impl SecureStore for MemoryStore {
    async fn get(&self, key: &str) -> Result<Option<Zeroizing<Vec<u8>>>, OvError> {
        Ok(self.0.lock().unwrap().get(key).cloned())
    }
    async fn set(&self, key: &str, value: &[u8]) -> Result<(), OvError> {
        self.0.lock().unwrap().insert(key.to_owned(), Zeroizing::new(value.to_vec()));
        Ok(())
    }
    async fn delete(&self, key: &str) -> Result<(), OvError> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
}

/// The offline cache. Everything in it is ciphertext or public, so an app keeps it in ordinary
/// files.
#[derive(Default)]
struct MemoryCache {
    vaults: Mutex<HashMap<(String, String), CachedVault>>,
    meta: Mutex<HashMap<(String, String), Vec<u8>>>,
}

#[async_trait::async_trait]
impl BlobCache for MemoryCache {
    async fn load(&self, user_id: &str, vault_id: &str) -> Result<Option<CachedVault>, OvError> {
        Ok(self.vaults.lock().unwrap().get(&(user_id.to_owned(), vault_id.to_owned())).cloned())
    }
    async fn save(&self, user_id: &str, vault_id: &str, v: &CachedVault) -> Result<(), OvError> {
        self.vaults.lock().unwrap().insert((user_id.to_owned(), vault_id.to_owned()), v.clone());
        Ok(())
    }
    async fn remove(&self, user_id: &str, vault_id: &str) -> Result<(), OvError> {
        self.vaults.lock().unwrap().remove(&(user_id.to_owned(), vault_id.to_owned()));
        Ok(())
    }
    async fn list(&self, user_id: &str) -> Result<Vec<String>, OvError> {
        Ok(self.vaults.lock().unwrap().keys().filter(|(u, _)| u == user_id).map(|(_, v)| v.clone()).collect())
    }
    async fn load_meta(&self, user_id: &str, name: &str) -> Result<Option<Vec<u8>>, OvError> {
        Ok(self.meta.lock().unwrap().get(&(user_id.to_owned(), name.to_owned())).cloned())
    }
    async fn save_meta(&self, user_id: &str, name: &str, value: &[u8]) -> Result<(), OvError> {
        self.meta.lock().unwrap().insert((user_id.to_owned(), name.to_owned()), value.to_vec());
        Ok(())
    }
    async fn clear_user(&self, user_id: &str) -> Result<(), OvError> {
        self.vaults.lock().unwrap().retain(|(u, _), _| u != user_id);
        self.meta.lock().unwrap().retain(|(u, _), _| u != user_id);
        Ok(())
    }
}

// --------------------------------------------------------------------------------- clock, logs

struct SystemClock;

impl Clock for SystemClock {
    fn now_unix_ms(&self) -> i64 {
        let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
    }
}

/// Warnings and errors on stderr; OV_LOG=debug for more. The core never logs secrets.
struct StderrLog;

impl LogSink for StderrLog {
    fn log(&self, level: Level, target: &str, msg: &str) {
        let max = match std::env::var("OV_LOG").as_deref() {
            Ok("debug") => Level::Debug,
            Ok("info") => Level::Info,
            _ => Level::Warn,
        };
        if level <= max {
            eprintln!("[{level:?} {target}] {msg}");
        }
    }
}
