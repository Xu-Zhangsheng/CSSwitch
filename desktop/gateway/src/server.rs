mod http_codec;
mod inference_dispatch;
mod skill_bridge_host;

use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use crate::config::GatewayConfig;
use crate::{codex_models, codex_transport, connect, models};

use self::http_codec::{invalid_request_json, not_found_json, read_head};
use self::inference_dispatch::{handle_get, handle_post, CodexComponents, RequestNonceGenerator};
use self::skill_bridge_host::start_skill_install_bridge;

const MAX_ACTIVE_CONNECTIONS: usize = 128;
const HTTP_WRITE_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

struct ConnectionBudget {
    active: Mutex<usize>,
    available: Condvar,
    limit: usize,
}

impl ConnectionBudget {
    fn new(limit: usize) -> Self {
        Self {
            active: Mutex::new(0),
            available: Condvar::new(),
            limit,
        }
    }

    fn acquire(self: &Arc<Self>) -> ConnectionPermit {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while *active >= self.limit {
            active = self
                .available
                .wait(active)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        *active += 1;
        ConnectionPermit {
            budget: Arc::clone(self),
        }
    }
}

struct ConnectionPermit {
    budget: Arc<ConnectionBudget>,
}

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        let mut active = self
            .budget
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *active = active.saturating_sub(1);
        self.budget.available.notify_one();
    }
}

fn handle_one(
    cfg: GatewayConfig,
    mut stream: TcpStream,
    request_nonces: Option<Arc<RequestNonceGenerator>>,
    relay_models: Arc<models::RelayModelCache>,
    codex_transport: Option<Arc<codex_transport::CodexTransport>>,
    codex_models: Option<Arc<codex_models::CodexModelCatalog>>,
) {
    let _ = stream.set_write_timeout(Some(HTTP_WRITE_IDLE_TIMEOUT));
    let head = match read_head(&mut stream) {
        Ok(head) => head,
        Err(e) => {
            invalid_request_json(&mut stream, &e);
            return;
        }
    };
    match head.method.as_str() {
        "CONNECT" => connect::handle_connect(&head.target, stream),
        "GET" => handle_get(
            &mut stream,
            &cfg,
            &head.target,
            &relay_models,
            codex_models.as_deref(),
        ),
        "POST" => {
            let target = head.target.clone();
            handle_post(
                &mut stream,
                &cfg,
                &target,
                &head,
                request_nonces.as_deref(),
                &relay_models,
                CodexComponents {
                    transport: codex_transport.as_deref(),
                    models: codex_models.as_deref(),
                },
            )
        }
        _ => not_found_json(&mut stream, &head.target),
    }
}

pub fn serve(cfg: GatewayConfig) -> Result<(), String> {
    let request_nonces = if cfg.provider == "deepseek" && cfg.shim_mode == "rewrite" {
        Some(Arc::new(RequestNonceGenerator::new()?))
    } else {
        None
    };
    let codex_transport = if cfg.provider == "codex" {
        let contract = cfg
            .codex_contract
            .as_ref()
            .ok_or("Codex provider contract is unavailable")?;
        Some(Arc::new(
            codex_transport::CodexTransport::production(contract)
                .map_err(|error| error.to_string())?,
        ))
    } else {
        None
    };
    let codex_models = if cfg.provider == "codex" {
        let contract = cfg
            .codex_contract
            .as_ref()
            .ok_or("Codex provider contract is unavailable")?;
        let state_root = cfg
            .codex_state_root
            .clone()
            .ok_or("Codex model catalog state root is unavailable")?;
        Some(Arc::new(
            codex_models::CodexModelCatalog::production(state_root, contract)
                .map_err(|error| error.to_string())?,
        ))
    } else {
        None
    };
    let relay_models = Arc::new(models::RelayModelCache::default());
    let connection_budget = Arc::new(ConnectionBudget::new(MAX_ACTIVE_CONNECTIONS));
    let listener = TcpListener::bind(("127.0.0.1", cfg.port)).map_err(|e| e.to_string())?;
    if let Err(error) = start_skill_install_bridge(&cfg) {
        eprintln!("Skill install host unavailable (gateway continues): {error}");
    }
    loop {
        // Reserve process resources before accept. Once the fixed budget is
        // exhausted, new peers remain in the kernel backlog instead of
        // creating unbounded handler/CONNECT threads and file descriptors.
        let permit = connection_budget.acquire();
        match listener.accept() {
            Ok((stream, _peer)) => {
                let cfg = cfg.clone();
                let request_nonces = request_nonces.clone();
                let relay_models = Arc::clone(&relay_models);
                let codex_transport = codex_transport.clone();
                let codex_models = codex_models.clone();
                thread::Builder::new()
                    .name("csswitch-gateway-connection".into())
                    .spawn(move || {
                        let _permit = permit;
                        handle_one(
                            cfg,
                            stream,
                            request_nonces,
                            relay_models,
                            codex_transport,
                            codex_models,
                        )
                    })
                    .map_err(|error| error.to_string())?;
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

#[cfg(test)]
#[path = "server/tests.rs"]
mod tests;
