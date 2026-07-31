//! Claude Science executable selection, managed identity, and lifecycle façade.
//!
//! The included fragments deliberately remain in this module so the historical
//! crate-facing surface, private helper relationships, failure codes, and test
//! identities stay unchanged while each responsibility has one file owner.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::Runtime;

use crate::{config, proc};

use super::system::{asset_root, kill_child};

// Keep include fragments on rustfmt's normal module-discovery path without
// changing the runtime module or the historical visibility surface.
#[cfg(any())]
#[path = "science/contracts.rs"]
mod format_contracts;
#[cfg(any())]
#[path = "science/executable.rs"]
mod format_executable;
#[cfg(any())]
#[path = "science/lifecycle.rs"]
mod format_lifecycle;
#[cfg(any())]
#[path = "science/managed_launch.rs"]
mod format_managed_launch;
#[cfg(any())]
#[path = "science/runtime_state.rs"]
mod format_runtime_state;

include!("science/contracts.rs");
include!("science/executable.rs");
include!("science/runtime_state.rs");
include!("science/managed_launch.rs");
include!("science/lifecycle.rs");

#[cfg(test)]
#[path = "science/tests.rs"]
mod tests;
