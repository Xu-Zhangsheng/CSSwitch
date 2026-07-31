mod http_codec;
mod inference_dispatch;
mod skill_bridge_host;

use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

use crate::config::GatewayConfig;
use crate::{codex_models, codex_transport, connect, models};

use self::http_codec::{invalid_request_json, not_found_json, read_head};
use self::inference_dispatch::{handle_get, handle_post, CodexComponents, RequestNonceGenerator};
use self::skill_bridge_host::start_skill_install_bridge;

fn handle_one(
    cfg: GatewayConfig,
    mut stream: TcpStream,
    request_nonces: Option<Arc<RequestNonceGenerator>>,
    relay_models: Arc<models::RelayModelCache>,
    codex_transport: Option<Arc<codex_transport::CodexTransport>>,
    codex_models: Option<Arc<codex_models::CodexModelCatalog>>,
) {
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
    let listener = TcpListener::bind(("127.0.0.1", cfg.port)).map_err(|e| e.to_string())?;
    if let Err(error) = start_skill_install_bridge(&cfg) {
        eprintln!("Skill install host unavailable (gateway continues): {error}");
    }
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let cfg = cfg.clone();
                let request_nonces = request_nonces.clone();
                let relay_models = Arc::clone(&relay_models);
                let codex_transport = codex_transport.clone();
                let codex_models = codex_models.clone();
                thread::spawn(move || {
                    handle_one(
                        cfg,
                        stream,
                        request_nonces,
                        relay_models,
                        codex_transport,
                        codex_models,
                    )
                });
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "server/tests.rs"]
mod tests;
