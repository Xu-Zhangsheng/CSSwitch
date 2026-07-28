# External CSSwitch control bridge

The control protocol is owned by `desktop/control-core`, not by Tauri or the
CSSwitch application bundle. The UI and the future external helper must both
use this crate for protocol versioning, configuration fingerprints, capability
advertising, and mutation envelopes.

The external helper must not call Tauri IPC, inspect WebView state, or proxy an
app-private HTTP endpoint. Those are implementation details and may change on
every CSSwitch update. The helper owns its listener, authentication record,
configuration transaction, and recovery journal. CSSwitch.app is only a UI
client of the same core.

The first extraction milestone is deliberately limited to configuration and
profile CRUD. Runtime start/stop and model discovery remain guarded until their
Rust transaction dependencies are moved into the shared core. Unsupported
operations must return a structured capability error; they must never claim a
successful write.
