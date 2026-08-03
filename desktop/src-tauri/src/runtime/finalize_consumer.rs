use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::{config, runtime::profile::selection_pending_from_config};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FinalizeConsumerDisposition {
    Ready,
    Attention,
    Manual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum JournalDisposition {
    Open,
    Cleared,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum BindingRelation {
    MatchesActive,
    DifferentActive,
    Absent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct FinalizeConsumerState {
    pub(crate) disposition: FinalizeConsumerDisposition,
    journal_disposition: JournalDisposition,
    binding_relation: BindingRelation,
    applied_profile_id: Option<String>,
    selection_pending: bool,
    cleanup_required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FinalizeReadModel {
    journal_disposition: JournalDisposition,
    binding_relation: BindingRelation,
    applied_profile_id: Option<String>,
    selection_pending: bool,
}

fn read_model(cfg: &config::Config) -> Result<FinalizeReadModel, String> {
    let applied_profile_id = cfg
        .runtime_binding
        .as_ref()
        .map(|binding| binding.profile_id.clone());
    let binding_relation = match applied_profile_id.as_deref() {
        None => BindingRelation::Absent,
        Some(profile_id) if !cfg.active_id.is_empty() && profile_id == cfg.active_id => {
            BindingRelation::MatchesActive
        }
        Some(_) => BindingRelation::DifferentActive,
    };
    Ok(FinalizeReadModel {
        journal_disposition: if cfg.runtime_transaction.is_some() {
            JournalDisposition::Open
        } else {
            JournalDisposition::Cleared
        },
        binding_relation,
        applied_profile_id,
        selection_pending: selection_pending_from_config(cfg)?,
    })
}

fn field<'a>(outcome: &'a Value, name: &str) -> Option<&'a str> {
    outcome.get(name).and_then(Value::as_str)
}

fn classify(outcome: &Value, readback: &FinalizeReadModel) -> FinalizeConsumerDisposition {
    let status = field(outcome, "status");
    let recovery = field(outcome, "recovery_status");
    let action = field(outcome, "action");

    if status == Some("error") || readback.journal_disposition == JournalDisposition::Open {
        return FinalizeConsumerDisposition::Manual;
    }

    if action == Some("history_choice_required") {
        return match (status, recovery) {
            (Some("attention"), Some("choice_required"))
            | (Some("degraded"), Some("cleanup_required"))
            | (Some("degraded"), Some("manual_recovery_required")) => {
                FinalizeConsumerDisposition::Attention
            }
            _ => FinalizeConsumerDisposition::Manual,
        };
    }

    let normal_action = matches!(action, Some("started" | "reopened"));
    let supported_outcome = matches!(
        (status, recovery),
        (Some("ok"), Some("not_needed" | "environment_uncertain"))
            | (
                Some("degraded"),
                Some("cleanup_required" | "manual_recovery_required")
            )
    );
    if normal_action
        && supported_outcome
        && readback.binding_relation == BindingRelation::MatchesActive
        && !readback.selection_pending
    {
        FinalizeConsumerDisposition::Ready
    } else {
        FinalizeConsumerDisposition::Manual
    }
}

pub(crate) fn project_finalize_consumer_state(
    dir: &Path,
    outcome: &Value,
) -> Result<FinalizeConsumerState, String> {
    let cfg = config::load_current_from_read_only(dir).map_err(|error| error.to_string())?;
    let readback = read_model(&cfg)?;
    Ok(project_read_model(outcome, readback))
}

fn project_read_model(outcome: &Value, readback: FinalizeReadModel) -> FinalizeConsumerState {
    let disposition = classify(outcome, &readback);
    FinalizeConsumerState {
        disposition,
        journal_disposition: readback.journal_disposition,
        binding_relation: readback.binding_relation,
        applied_profile_id: (disposition == FinalizeConsumerDisposition::Ready)
            .then_some(readback.applied_profile_id)
            .flatten(),
        selection_pending: if disposition == FinalizeConsumerDisposition::Ready {
            readback.selection_pending
        } else {
            true
        },
        cleanup_required: field(outcome, "recovery_status") == Some("cleanup_required"),
    }
}

#[cfg(test)]
pub(crate) fn test_consumer_state(
    disposition: FinalizeConsumerDisposition,
) -> FinalizeConsumerState {
    let ready = disposition == FinalizeConsumerDisposition::Ready;
    FinalizeConsumerState {
        disposition,
        journal_disposition: JournalDisposition::Cleared,
        binding_relation: BindingRelation::MatchesActive,
        applied_profile_id: ready.then(|| "test-profile".into()),
        selection_pending: !ready,
        cleanup_required: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn model(
        journal_disposition: JournalDisposition,
        binding_relation: BindingRelation,
        selection_pending: bool,
    ) -> FinalizeReadModel {
        FinalizeReadModel {
            journal_disposition,
            binding_relation,
            applied_profile_id: (binding_relation != BindingRelation::Absent)
                .then(|| "public-profile-id".to_string()),
            selection_pending,
        }
    }

    fn outcome(status: &str, recovery: &str, action: &str) -> Value {
        serde_json::json!({
            "status": status,
            "recovery_status": recovery,
            "action": action,
            "message": "must not control classification",
            "cleanup_recovery_path": "/private/secret/must-not-project",
        })
    }

    #[test]
    fn h4_classifier_covers_status_recovery_action_and_readback_counterexamples() {
        let cleared_match = model(
            JournalDisposition::Cleared,
            BindingRelation::MatchesActive,
            false,
        );
        let cleared_absent = model(JournalDisposition::Cleared, BindingRelation::Absent, true);
        let open_match = model(
            JournalDisposition::Open,
            BindingRelation::MatchesActive,
            true,
        );

        for ready in [
            outcome("ok", "not_needed", "started"),
            outcome("degraded", "cleanup_required", "started"),
            outcome("degraded", "manual_recovery_required", "started"),
        ] {
            assert_eq!(
                classify(&ready, &cleared_match),
                FinalizeConsumerDisposition::Ready
            );
        }
        for attention in [
            outcome("attention", "choice_required", "history_choice_required"),
            outcome("degraded", "cleanup_required", "history_choice_required"),
            outcome(
                "degraded",
                "manual_recovery_required",
                "history_choice_required",
            ),
        ] {
            assert_eq!(
                classify(&attention, &cleared_absent),
                FinalizeConsumerDisposition::Attention
            );
        }
        let history_with_existing_binding = project_read_model(
            &outcome("degraded", "cleanup_required", "history_choice_required"),
            cleared_match.clone(),
        );
        assert_eq!(
            history_with_existing_binding.disposition,
            FinalizeConsumerDisposition::Attention
        );
        assert_eq!(history_with_existing_binding.applied_profile_id, None);
        assert!(history_with_existing_binding.selection_pending);
        assert_eq!(
            classify(
                &outcome("degraded", "manual_recovery_required", "started"),
                &open_match,
            ),
            FinalizeConsumerDisposition::Manual
        );
        assert_eq!(
            classify(
                &outcome("error", "manual_recovery_required", "started"),
                &cleared_match,
            ),
            FinalizeConsumerDisposition::Manual
        );
        assert_eq!(
            classify(
                &outcome("degraded", "cleanup_required", "started"),
                &cleared_absent,
            ),
            FinalizeConsumerDisposition::Manual
        );
    }

    #[test]
    fn manual_recovery_classifier_uses_readback_instead_of_assuming_atomic_outcome() {
        let degraded = outcome("degraded", "manual_recovery_required", "started");
        let possible_old_state = model(
            JournalDisposition::Open,
            BindingRelation::DifferentActive,
            true,
        );
        let possible_committed_state = model(
            JournalDisposition::Cleared,
            BindingRelation::MatchesActive,
            false,
        );
        assert_eq!(
            classify(&degraded, &possible_old_state),
            FinalizeConsumerDisposition::Manual
        );
        assert_eq!(
            classify(&degraded, &possible_committed_state),
            FinalizeConsumerDisposition::Ready
        );
    }

    #[test]
    fn finalize_projection_is_read_only_and_serializes_no_journal_or_secret_fields() {
        let dir = std::env::temp_dir().join(format!(
            "csswitch-h4-finalize-projection-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        config::save_to(&dir, &config::Config::default()).unwrap();
        let before = std::fs::read(dir.join("config.json")).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o750)).unwrap();
        std::fs::set_permissions(
            dir.join("config.json"),
            std::fs::Permissions::from_mode(0o640),
        )
        .unwrap();
        let dir_mode_before = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        let file_mode_before = std::fs::metadata(dir.join("config.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        let projected = project_finalize_consumer_state(
            &dir,
            &outcome("error", "manual_recovery_required", "failed"),
        )
        .unwrap();
        let value = serde_json::to_value(projected).unwrap();
        assert_eq!(value["disposition"], "manual");
        assert_eq!(value["journal_disposition"], "cleared");
        assert_eq!(value["binding_relation"], "absent");
        assert!(value["applied_profile_id"].is_null());
        assert_eq!(value["selection_pending"], true);
        assert_eq!(std::fs::read(dir.join("config.json")).unwrap(), before);
        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            dir_mode_before
        );
        assert_eq!(
            std::fs::metadata(dir.join("config.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            file_mode_before
        );
        let surface = value.to_string();
        for forbidden in [
            "runtime_transaction",
            "transaction_id",
            "snapshot_ticket",
            "cleanup_recovery_path",
            "/private/secret",
            "api_key",
            "credential",
        ] {
            assert!(
                !surface.contains(forbidden),
                "leaked {forbidden}: {surface}"
            );
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
