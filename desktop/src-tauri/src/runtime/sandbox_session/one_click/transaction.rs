use std::path::Path;

use crate::config;

#[cfg(test)]
use super::super::authority_snapshot::SANDBOX_SESSION_TEST_SEAMS;
use super::super::recovery::RuntimeTransactionRestoreExpectation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct OneClickTransactionIdentity {
    pub(in super::super) target_profile_id: String,
    pub(in super::super) runtime_fingerprint: String,
    pub(in super::super) snapshot_ticket: config::RuntimeSnapshotTicket,
    pub(in super::super) previous_binding: Option<config::RuntimeBindingCommit>,
    pub(in super::super) gateway_terminal_handoff: Option<config::RuntimeTransactionV2>,
    pub(in super::super) prior_stop: config::RuntimePriorStopState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) enum OneClickJournalProgress {
    PreJournalAbort {
        registered_ticket: config::RuntimeSnapshotTicket,
        runtime_transaction: Box<Option<config::RuntimeTransactionRecord>>,
    },
    Journaled {
        record: config::RuntimeTransactionV2,
        registered_ticket: config::RuntimeSnapshotTicket,
    },
    Compensating {
        active_runtime_transaction: Box<Option<config::RuntimeTransactionRecord>>,
        restored_runtime_transaction: Box<Option<config::RuntimeTransactionRecord>>,
        expected_runtime_transaction: Box<Option<config::RuntimeTransactionRecord>>,
        compensation: Box<config::RuntimeCompensationJournal>,
        registered_ticket: config::RuntimeSnapshotTicket,
    },
    CompensationFinished {
        registered_ticket: config::RuntimeSnapshotTicket,
    },
    Finalized {
        record: config::RuntimeTransactionV2,
        registered_ticket: config::RuntimeSnapshotTicket,
    },
}

impl OneClickJournalProgress {
    fn registered_ticket(&self) -> &config::RuntimeSnapshotTicket {
        match self {
            Self::PreJournalAbort {
                registered_ticket, ..
            }
            | Self::Journaled {
                registered_ticket, ..
            }
            | Self::Compensating {
                registered_ticket, ..
            }
            | Self::CompensationFinished { registered_ticket }
            | Self::Finalized {
                registered_ticket, ..
            } => registered_ticket,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in super::super) fn transaction_id(&self) -> Option<&str> {
        match self {
            Self::PreJournalAbort {
                runtime_transaction,
                ..
            } => runtime_transaction
                .as_ref()
                .as_ref()
                .map(config::RuntimeTransactionRecord::transaction_id),
            Self::Journaled { record, .. } | Self::Finalized { record, .. } => {
                Some(&record.transaction_id)
            }
            Self::Compensating {
                active_runtime_transaction,
                ..
            } => active_runtime_transaction
                .as_ref()
                .as_ref()
                .map(config::RuntimeTransactionRecord::transaction_id),
            Self::CompensationFinished { .. } => None,
        }
    }

    pub(in super::super) fn journaled_record(&self) -> Option<&config::RuntimeTransactionV2> {
        match self {
            Self::PreJournalAbort { .. }
            | Self::Compensating { .. }
            | Self::CompensationFinished { .. }
            | Self::Finalized { .. } => None,
            Self::Journaled { record, .. } => Some(record),
        }
    }

    pub(super) fn restore_expectation(&self) -> RuntimeTransactionRestoreExpectation {
        match self {
            Self::PreJournalAbort {
                runtime_transaction,
                ..
            } => RuntimeTransactionRestoreExpectation::Exact(runtime_transaction.as_ref().clone()),
            Self::Journaled { record, .. } => RuntimeTransactionRestoreExpectation::Exact(Some(
                config::RuntimeTransactionRecord::V2(record.clone()),
            )),
            Self::Compensating {
                expected_runtime_transaction,
                compensation,
                ..
            } => RuntimeTransactionRestoreExpectation::ExactPreservingCompensation {
                runtime_transaction: expected_runtime_transaction.as_ref().clone(),
                compensation: compensation.as_ref().clone(),
            },
            Self::CompensationFinished { .. } => RuntimeTransactionRestoreExpectation::Exact(None),
            Self::Finalized { .. } => RuntimeTransactionRestoreExpectation::Exact(None),
        }
    }
}

pub(in super::super) fn one_click_phase_exposure(
    phase: config::RuntimeTransactionPhase,
) -> config::RuntimeEnvironmentExposure {
    match phase {
        config::RuntimeTransactionPhase::StartScienceEnvironmentPending => {
            config::RuntimeEnvironmentExposure::Possible
        }
        config::RuntimeTransactionPhase::WaitScienceDbReverify
        | config::RuntimeTransactionPhase::RestartScienceAfterDbHeal
        | config::RuntimeTransactionPhase::VerifyScienceDbAfterRestart
        | config::RuntimeTransactionPhase::VerifyScienceCatalog => {
            config::RuntimeEnvironmentExposure::Exposed
        }
        _ => config::RuntimeEnvironmentExposure::NotExposed,
    }
}

fn one_click_journal_matches(
    journal: &config::RuntimeTransactionV2,
    identity: &OneClickTransactionIdentity,
    transaction_id: &str,
) -> bool {
    journal.schema_version == config::RUNTIME_TRANSACTION_SCHEMA_VERSION_V2
        && journal.transaction_id == transaction_id
        && journal.operation == config::RuntimeTransactionOperation::OneClick
        && journal.target_profile_id == identity.target_profile_id
        && journal.runtime_fingerprint.as_deref() == Some(&identity.runtime_fingerprint)
        && journal.snapshot_ticket.as_ref() == Some(&identity.snapshot_ticket)
        && journal.previous_binding.as_ref() == identity.previous_binding.as_ref()
        && journal.previous_gateway.is_none()
        && journal.compensation == config::RuntimeCompensationState::NotStarted
        && journal.gateway_stop_outcome == config::RuntimeGatewayStopOutcome::NotAttempted
        && journal.environment_exposure == one_click_phase_exposure(journal.phase)
        && journal.prior_stop == identity.prior_stop
        && journal.finalize == config::RuntimeFinalizeState::NotStarted
}

fn one_click_prior_stop_record_matches(
    journal: &config::RuntimeTransactionV2,
    identity: &OneClickTransactionIdentity,
    transaction_id: &str,
) -> bool {
    journal.schema_version == config::RUNTIME_TRANSACTION_SCHEMA_VERSION_V2
        && journal.transaction_id == transaction_id
        && journal.operation == config::RuntimeTransactionOperation::OneClick
        && journal.target_profile_id == identity.target_profile_id
        && journal.phase == config::RuntimeTransactionPhase::StopOldScience
        && journal.runtime_fingerprint.as_deref() == Some(&identity.runtime_fingerprint)
        && journal.snapshot_ticket.is_none()
        && journal.previous_binding.as_ref() == identity.previous_binding.as_ref()
        && journal.previous_gateway.is_none()
        && journal.compensation == config::RuntimeCompensationState::NotStarted
        && journal.gateway_stop_outcome == config::RuntimeGatewayStopOutcome::NotAttempted
        && journal.environment_exposure == config::RuntimeEnvironmentExposure::NotExposed
        && journal.prior_stop == identity.prior_stop
        && matches!(
            journal.prior_stop,
            config::RuntimePriorStopState::Outcome {
                outcome: config::RuntimePriorStopOutcome::ExactStopped,
                ..
            }
        )
        && journal.finalize == config::RuntimeFinalizeState::NotStarted
}

pub(super) fn config_authority_matches(
    current: &config::Config,
    target_profile_id: &str,
    previous_binding: Option<&config::RuntimeBindingCommit>,
) -> bool {
    current.active_id == target_profile_id && current.runtime_binding.as_ref() == previous_binding
}

fn gateway_terminal_handoff_matches(
    journal: &config::RuntimeTransactionV2,
    expected: &config::RuntimeTransactionV2,
    active_profile_id: &str,
    current_binding: Option<&config::RuntimeBindingCommit>,
) -> bool {
    journal == expected
        && expected.schema_version == config::RUNTIME_TRANSACTION_SCHEMA_VERSION_V2
        && expected.operation == config::RuntimeTransactionOperation::ProfileSwitch
        && expected.target_profile_id == active_profile_id
        && expected.phase == config::RuntimeTransactionPhase::RecoverInterruptedGateway
        && expected.runtime_fingerprint.is_none()
        && expected.snapshot_ticket.is_none()
        && expected.previous_binding.as_ref() == current_binding
        && expected.environment_exposure == config::RuntimeEnvironmentExposure::NotExposed
        && expected.compensation == config::RuntimeCompensationState::NotStarted
        && matches!(
            expected.gateway_stop_outcome,
            config::RuntimeGatewayStopOutcome::Stopped
                | config::RuntimeGatewayStopOutcome::AbsentAfterAttempt
        )
        && expected.prior_stop == config::RuntimePriorStopState::NotRequired
        && expected.finalize == config::RuntimeFinalizeState::NotStarted
}

pub(in super::super) fn resolve_gateway_terminal_handoff(
    journal: Option<&config::RuntimeTransactionRecord>,
    expected: Option<&config::RuntimeTransactionV2>,
    active_profile_id: &str,
    current_binding: Option<&config::RuntimeBindingCommit>,
) -> Result<Option<config::RuntimeTransactionV2>, &'static str> {
    match expected {
        Some(expected) => match journal {
            Some(config::RuntimeTransactionRecord::V2(typed))
                if gateway_terminal_handoff_matches(
                    typed,
                    expected,
                    active_profile_id,
                    current_binding,
                ) =>
            {
                Ok(Some(expected.clone()))
            }
            _ => Err("interrupted-Gateway terminal handoff disappeared, drifted, or retargeted"),
        },
        None => Ok(None),
    }
}

pub(in super::super) fn healthy_reopen_transaction_matches(
    journal: Option<&config::RuntimeTransactionRecord>,
    expected_gateway_terminal_handoff: Option<&config::RuntimeTransactionV2>,
    active_profile_id: &str,
    current_binding: Option<&config::RuntimeBindingCommit>,
) -> bool {
    if let Some(expected) = expected_gateway_terminal_handoff {
        return matches!(
            journal,
            Some(config::RuntimeTransactionRecord::V2(typed))
                if gateway_terminal_handoff_matches(
                    typed,
                    expected,
                    active_profile_id,
                    current_binding,
                )
        );
    }
    !journal.is_some_and(config::RuntimeTransactionRecord::is_v2)
}

pub(in super::super) fn begin_healthy_reopen_gateway_intent(
    dir: &Path,
    expected_config: &config::Config,
    expected_gateway_terminal_handoff: Option<&config::RuntimeTransactionV2>,
) -> Result<config::RuntimeTransactionV2, String> {
    config::update_result(dir, |current| {
        if current != expected_config
            || !healthy_reopen_transaction_matches(
                current.runtime_transaction.as_ref(),
                expected_gateway_terminal_handoff,
                &current.active_id,
                current.runtime_binding.as_ref(),
            )
        {
            return Err(
                "healthy reopen admission found Config drift or a retargeted runtime handoff"
                    .into(),
            );
        }
        let record = config::RuntimeTransactionV2 {
            schema_version: config::RUNTIME_TRANSACTION_SCHEMA_VERSION_V2,
            transaction_id: config::new_id(),
            operation: config::RuntimeTransactionOperation::ProfileSwitch,
            target_profile_id: current.active_id.clone(),
            phase: config::RuntimeTransactionPhase::StartFormalGateway,
            runtime_fingerprint: None,
            environment_exposure: config::RuntimeEnvironmentExposure::NotExposed,
            snapshot_ticket: None,
            previous_binding: current.runtime_binding.clone(),
            previous_gateway: expected_gateway_terminal_handoff
                .and_then(|handoff| handoff.previous_gateway.clone()),
            compensation: config::RuntimeCompensationState::NotStarted,
            gateway_stop_outcome: config::RuntimeGatewayStopOutcome::NotAttempted,
            prior_stop: config::RuntimePriorStopState::NotRequired,
            finalize: config::RuntimeFinalizeState::NotStarted,
        };
        current.runtime_transaction = Some(config::RuntimeTransactionRecord::V2(record.clone()));
        Ok((record, true))
    })
}

pub(in super::super) fn commit_healthy_reopen_binding(
    dir: &Path,
    expected_gateway_intent: &config::RuntimeTransactionV2,
    committed: &config::RuntimeBindingCommit,
) -> Result<(), String> {
    config::update_result(dir, |config| {
        if config.runtime_transaction.as_ref()
            != Some(&config::RuntimeTransactionRecord::V2(
                expected_gateway_intent.clone(),
            ))
            || config.active_id != expected_gateway_intent.target_profile_id
            || config.runtime_binding.as_ref() != expected_gateway_intent.previous_binding.as_ref()
        {
            return Err(
                "healthy reopen Gateway intent drifted; preserved the current transaction".into(),
            );
        }
        config.runtime_binding = Some(committed.clone());
        config.runtime_transaction = None;
        Ok(((), true))
    })
}

fn new_one_click_journal(
    identity: &OneClickTransactionIdentity,
    transaction_id: String,
    phase: config::RuntimeTransactionPhase,
) -> config::RuntimeTransactionV2 {
    config::RuntimeTransactionV2 {
        schema_version: config::RUNTIME_TRANSACTION_SCHEMA_VERSION_V2,
        transaction_id,
        operation: config::RuntimeTransactionOperation::OneClick,
        target_profile_id: identity.target_profile_id.clone(),
        phase,
        runtime_fingerprint: Some(identity.runtime_fingerprint.clone()),
        environment_exposure: one_click_phase_exposure(phase),
        snapshot_ticket: Some(identity.snapshot_ticket.clone()),
        previous_binding: identity.previous_binding.clone(),
        previous_gateway: None,
        compensation: config::RuntimeCompensationState::NotStarted,
        gateway_stop_outcome: config::RuntimeGatewayStopOutcome::NotAttempted,
        prior_stop: identity.prior_stop.clone(),
        finalize: config::RuntimeFinalizeState::NotStarted,
    }
}

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn begin_prior_stop_intent(
    dir: &Path,
    target_profile_id: &str,
    runtime_fingerprint: &str,
    previous_binding: Option<&config::RuntimeBindingCommit>,
    gateway_terminal_handoff: Option<&config::RuntimeTransactionV2>,
    recipe: config::RuntimePriorScienceRecipe,
) -> Result<config::RuntimeTransactionV2, String> {
    config::update_result(dir, |current| {
        let current_handoff_matches = match current.runtime_transaction.as_ref() {
            Some(config::RuntimeTransactionRecord::V2(journal)) => gateway_terminal_handoff
                .is_some_and(|expected| {
                    gateway_terminal_handoff_matches(
                        journal,
                        expected,
                        &current.active_id,
                        current.runtime_binding.as_ref(),
                    )
                }),
            _ => false,
        };
        let config_authority_matches =
            config_authority_matches(current, target_profile_id, previous_binding);
        let no_handoff_expected = gateway_terminal_handoff.is_none();
        let replace_allowed = config_authority_matches
            && (current_handoff_matches
                || (no_handoff_expected
                    && matches!(
                        current.runtime_transaction.as_ref(),
                        None | Some(config::RuntimeTransactionRecord::V1(_))
                    )));
        if !replace_allowed {
            return Err(
                "prior Science stop intent found a drifted runtime handoff; preserved the current transaction"
                    .into(),
            );
        }
        let record = config::RuntimeTransactionV2 {
            schema_version: config::RUNTIME_TRANSACTION_SCHEMA_VERSION_V2,
            transaction_id: config::new_id(),
            operation: config::RuntimeTransactionOperation::OneClick,
            target_profile_id: target_profile_id.to_string(),
            phase: config::RuntimeTransactionPhase::StopOldScience,
            runtime_fingerprint: Some(runtime_fingerprint.to_string()),
            environment_exposure: config::RuntimeEnvironmentExposure::NotExposed,
            snapshot_ticket: None,
            previous_binding: previous_binding.cloned(),
            previous_gateway: None,
            compensation: config::RuntimeCompensationState::NotStarted,
            gateway_stop_outcome: config::RuntimeGatewayStopOutcome::NotAttempted,
            prior_stop: config::RuntimePriorStopState::Intent { recipe },
            finalize: config::RuntimeFinalizeState::NotStarted,
        };
        current.runtime_transaction = Some(config::RuntimeTransactionRecord::V2(record.clone()));
        Ok((record, true))
    })
}

pub(in super::super) fn publish_prior_stop_outcome(
    dir: &Path,
    expected: &config::RuntimeTransactionV2,
    outcome: config::RuntimePriorStopOutcome,
) -> Result<config::RuntimeTransactionV2, String> {
    let recipe = match &expected.prior_stop {
        config::RuntimePriorStopState::Intent { recipe } => recipe.clone(),
        _ => return Err("prior Science stop outcome has no matching durable intent".into()),
    };
    config::update_result(dir, |current| {
        if !config_authority_matches(
            current,
            &expected.target_profile_id,
            expected.previous_binding.as_ref(),
        ) || current.runtime_transaction.as_ref()
            != Some(&config::RuntimeTransactionRecord::V2(expected.clone()))
        {
            return Err(
                "prior Science stop outcome found a drifted intent; preserved the current transaction"
                    .into(),
            );
        }
        let mut record = expected.clone();
        record.prior_stop = config::RuntimePriorStopState::Outcome { recipe, outcome };
        current.runtime_transaction = Some(config::RuntimeTransactionRecord::V2(record.clone()));
        Ok((record, true))
    })
}

pub(super) fn clear_prior_stop_transition(
    dir: &Path,
    expected: &config::RuntimeTransactionV2,
) -> Result<(), String> {
    config::update_result(dir, |current| {
        if !config_authority_matches(
            current,
            &expected.target_profile_id,
            expected.previous_binding.as_ref(),
        ) || current.runtime_transaction.as_ref()
            != Some(&config::RuntimeTransactionRecord::V2(expected.clone()))
        {
            return Err(
                "prior Science restart found a drifted stop outcome; preserved the current transaction"
                    .into(),
            );
        }
        current.runtime_transaction = None;
        Ok(((), true))
    })
}

pub(in super::super) fn write_one_click_checkpoint(
    dir: &Path,
    identity: &OneClickTransactionIdentity,
    progress: &mut OneClickJournalProgress,
    phase: config::RuntimeTransactionPhase,
) -> Result<(), String> {
    if progress.registered_ticket() != &identity.snapshot_ticket {
        return Err("one-click checkpoint rejected a replaced in-memory snapshot ticket".into());
    }
    #[cfg(test)]
    if progress.journaled_record().is_none()
        || progress
            .journaled_record()
            .is_some_and(|record| record.snapshot_ticket.is_none())
    {
        let mut seams = SANDBOX_SESSION_TEST_SEAMS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if seams.one_click_fail_first_journal.as_deref() == Some(dir) {
            seams.one_click_fail_first_journal = None;
            return Err("test-only one-click first journal write failure".into());
        }
    }
    let expected_record = progress.journaled_record().cloned();
    let next_record = config::update_result(dir, |current| {
        let gateway_terminal_handoff_matches = match current.runtime_transaction.as_ref() {
            Some(config::RuntimeTransactionRecord::V2(journal)) => identity
                .gateway_terminal_handoff
                .as_ref()
                .is_some_and(|expected| {
                    gateway_terminal_handoff_matches(
                        journal,
                        expected,
                        &current.active_id,
                        current.runtime_binding.as_ref(),
                    )
                }),
            _ => false,
        };
        if !config_authority_matches(
            current,
            &identity.target_profile_id,
            identity.previous_binding.as_ref(),
        ) {
            return Err(
                "one-click checkpoint config authority drifted; preserved the typed transaction"
                    .into(),
            );
        }
        match (
            current.runtime_transaction.as_ref(),
            expected_record.as_ref(),
        ) {
            (Some(config::RuntimeTransactionRecord::V2(journal)), Some(expected))
                if journal == expected
                    && (one_click_journal_matches(journal, identity, &expected.transaction_id)
                        || one_click_prior_stop_record_matches(
                            journal,
                            identity,
                            &expected.transaction_id,
                        )) =>
            {
                let mut next = expected.clone();
                next.phase = phase;
                next.environment_exposure = one_click_phase_exposure(phase);
                next.snapshot_ticket = Some(identity.snapshot_ticket.clone());
                current.runtime_transaction =
                    Some(config::RuntimeTransactionRecord::V2(next.clone()));
                Ok((next, true))
            }
            (Some(config::RuntimeTransactionRecord::V2(_)), None)
                if gateway_terminal_handoff_matches =>
            {
                let transaction_id = config::new_id();
                let next = new_one_click_journal(identity, transaction_id, phase);
                current.runtime_transaction =
                    Some(config::RuntimeTransactionRecord::V2(next.clone()));
                Ok((next, true))
            }
            (Some(config::RuntimeTransactionRecord::V2(_)), _) => {
                Err("one-click checkpoint identity changed; preserved the typed transaction".into())
            }
            (Some(config::RuntimeTransactionRecord::V1(_)) | None, None)
                if identity.gateway_terminal_handoff.is_none() =>
            {
                let transaction_id = config::new_id();
                let next = new_one_click_journal(identity, transaction_id, phase);
                current.runtime_transaction =
                    Some(config::RuntimeTransactionRecord::V2(next.clone()));
                Ok((next, true))
            }
            (Some(config::RuntimeTransactionRecord::V1(_)) | None, None) => {
                Err("runtime handoff journal disappeared or regressed; refused replacement".into())
            }
            (Some(config::RuntimeTransactionRecord::V1(_)) | None, Some(_)) => Err(
                "one-click checkpoint journal disappeared or regressed; refused replacement".into(),
            ),
        }
    })?;
    *progress = OneClickJournalProgress::Journaled {
        record: next_record,
        registered_ticket: identity.snapshot_ticket.clone(),
    };
    Ok(())
}

#[cfg(test)]
pub(in super::super) fn begin_one_click_compensation(
    dir: &Path,
    identity: &OneClickTransactionIdentity,
    progress: &mut OneClickJournalProgress,
    restored_runtime_transaction: Option<config::RuntimeTransactionRecord>,
) -> Result<(), String> {
    begin_one_click_compensation_with_id(
        dir,
        identity,
        progress,
        restored_runtime_transaction,
        config::new_id(),
        Vec::new(),
    )
}

pub(super) fn begin_one_click_compensation_with_id(
    dir: &Path,
    identity: &OneClickTransactionIdentity,
    progress: &mut OneClickJournalProgress,
    restored_runtime_transaction: Option<config::RuntimeTransactionRecord>,
    compensation_id: String,
    science_adoption_attempt_ids: Vec<String>,
) -> Result<(), String> {
    let registered_ticket = progress.registered_ticket().clone();
    let active_runtime_transaction = match progress {
        OneClickJournalProgress::PreJournalAbort {
            runtime_transaction,
            ..
        } => runtime_transaction.as_ref().clone(),
        OneClickJournalProgress::Journaled { record, .. } => {
            let snapshot_identity_matches = record.snapshot_ticket.as_ref()
                == Some(&registered_ticket)
                || (record.snapshot_ticket.is_none()
                    && one_click_prior_stop_record_matches(
                        record,
                        identity,
                        &record.transaction_id,
                    ));
            if record.operation != config::RuntimeTransactionOperation::OneClick
                || record.runtime_fingerprint.as_deref()
                    != Some(identity.runtime_fingerprint.as_str())
                || !snapshot_identity_matches
                || record.previous_gateway.is_some()
                || record.compensation != config::RuntimeCompensationState::NotStarted
                || record.gateway_stop_outcome != config::RuntimeGatewayStopOutcome::NotAttempted
                || record.environment_exposure != one_click_phase_exposure(record.phase)
                || record.finalize != config::RuntimeFinalizeState::NotStarted
            {
                return Err("one-click compensation intent rejected an ineligible journal".into());
            }
            Some(config::RuntimeTransactionRecord::V2(record.clone()))
        }
        OneClickJournalProgress::Compensating { .. }
        | OneClickJournalProgress::CompensationFinished { .. }
        | OneClickJournalProgress::Finalized { .. } => {
            return Err("one-click compensation intent was already published".into())
        }
    };
    if identity.snapshot_ticket != registered_ticket {
        return Err("one-click compensation intent rejected a replaced snapshot ticket".into());
    }
    let _publication_lease = config::acquire_runtime_compensation_publication_lease(dir)
        .map_err(|error| format!("one-click compensation publication fence failed: {error}"))?;
    let compensation = config::update_result(dir, |current| {
        if !config_authority_matches(
            current,
            &identity.target_profile_id,
            identity.previous_binding.as_ref(),
        ) || current.runtime_transaction != active_runtime_transaction
            || current.runtime_compensation.is_some()
        {
            return Err(
                "one-click compensation intent found a drifted runtime journal; preserved the current transaction"
                    .into(),
            );
        }
        let next = config::RuntimeCompensationJournal {
            schema_version: config::RUNTIME_COMPENSATION_SCHEMA_VERSION_V2,
            compensation_id: compensation_id.clone(),
            target_profile_id: identity.target_profile_id.clone(),
            runtime_fingerprint: identity.runtime_fingerprint.clone(),
            snapshot_ticket: identity.snapshot_ticket.clone(),
            state: config::RuntimeCompensationState::InProgress,
            steps: config::pending_one_click_compensation_steps(),
            science_adoption_attempt_ids: science_adoption_attempt_ids.clone(),
        };
        current.runtime_compensation = Some(next.clone());
        Ok((next, true))
    })?;
    *progress = OneClickJournalProgress::Compensating {
        expected_runtime_transaction: Box::new(active_runtime_transaction.clone()),
        active_runtime_transaction: Box::new(active_runtime_transaction),
        restored_runtime_transaction: Box::new(restored_runtime_transaction),
        compensation: Box::new(compensation),
        registered_ticket,
    };
    Ok(())
}

fn update_one_click_compensation_step(
    dir: &Path,
    progress: &mut OneClickJournalProgress,
    step: config::RuntimeCompensationStep,
    expected_outcome: config::RuntimeCompensationStepState,
    next_outcome: config::RuntimeCompensationStepState,
) -> Result<(), String> {
    let (
        active_runtime_transaction,
        restored_runtime_transaction,
        expected_runtime_transaction,
        expected,
        registered_ticket,
    ) = match progress {
        OneClickJournalProgress::Compensating {
            active_runtime_transaction,
            restored_runtime_transaction,
            expected_runtime_transaction,
            compensation,
            registered_ticket,
        } => (
            active_runtime_transaction.as_ref().clone(),
            restored_runtime_transaction.as_ref().clone(),
            expected_runtime_transaction.as_ref().clone(),
            compensation.as_ref().clone(),
            registered_ticket.clone(),
        ),
        _ => return Err("one-click compensation step has no durable intent".into()),
    };
    if expected.schema_version != config::RUNTIME_COMPENSATION_SCHEMA_VERSION_V2
        || expected.state != config::RuntimeCompensationState::InProgress
    {
        return Err("one-click compensation step requires an active V2 journal".into());
    }
    let transition_valid = matches!(
        (expected_outcome, next_outcome),
        (
            config::RuntimeCompensationStepState::Pending,
            config::RuntimeCompensationStepState::InProgress
        )
    ) || (expected_outcome
        == config::RuntimeCompensationStepState::InProgress
        && next_outcome.is_terminal());
    if !transition_valid {
        return Err("one-click compensation step transition is invalid".into());
    }
    let Some(step_index) = expected
        .steps
        .iter()
        .position(|candidate| candidate.step == step)
    else {
        return Err("one-click compensation step is not part of the durable plan".into());
    };
    if expected.steps[step_index].outcome != expected_outcome
        || expected.steps[..step_index]
            .iter()
            .any(|candidate| !candidate.outcome.is_terminal())
        || expected.steps[step_index + 1..]
            .iter()
            .any(|candidate| candidate.outcome != config::RuntimeCompensationStepState::Pending)
    {
        return Err("one-click compensation step rejected non-canonical progress".into());
    }
    let next = config::update_result(dir, |current| {
        if current.runtime_compensation.as_ref() != Some(&expected)
            || current.runtime_transaction != expected_runtime_transaction
        {
            return Err(
                "one-click compensation step found a drifted runtime journal; preserved the current transaction"
                    .into(),
            );
        }
        let mut record = expected.clone();
        record.steps[step_index].outcome = next_outcome;
        current.runtime_compensation = Some(record.clone());
        Ok((record, true))
    })?;
    *progress = OneClickJournalProgress::Compensating {
        active_runtime_transaction: Box::new(active_runtime_transaction),
        restored_runtime_transaction: Box::new(restored_runtime_transaction),
        expected_runtime_transaction: Box::new(expected_runtime_transaction),
        compensation: Box::new(next),
        registered_ticket,
    };
    Ok(())
}

pub(in super::super) fn begin_one_click_compensation_step(
    dir: &Path,
    progress: &mut OneClickJournalProgress,
    step: config::RuntimeCompensationStep,
) -> Result<(), String> {
    update_one_click_compensation_step(
        dir,
        progress,
        step,
        config::RuntimeCompensationStepState::Pending,
        config::RuntimeCompensationStepState::InProgress,
    )
}

pub(in super::super) fn finish_one_click_compensation_step(
    dir: &Path,
    progress: &mut OneClickJournalProgress,
    step: config::RuntimeCompensationStep,
    outcome: config::RuntimeCompensationStepState,
) -> Result<(), String> {
    if step == config::RuntimeCompensationStep::AuthorityRestore {
        return Err(
            "authority restore outcome requires the dedicated business-record transition".into(),
        );
    }
    update_one_click_compensation_step(
        dir,
        progress,
        step,
        config::RuntimeCompensationStepState::InProgress,
        outcome,
    )
}

pub(in super::super) fn finish_one_click_authority_restore_step(
    dir: &Path,
    progress: &mut OneClickJournalProgress,
    outcome: config::RuntimeCompensationStepState,
) -> Result<(), String> {
    if !outcome.is_terminal() {
        return Err("authority restore outcome must be terminal".into());
    }
    let (
        active_runtime_transaction,
        restored_runtime_transaction,
        expected_runtime_transaction,
        expected,
        registered_ticket,
    ) = match progress {
        OneClickJournalProgress::Compensating {
            active_runtime_transaction,
            restored_runtime_transaction,
            expected_runtime_transaction,
            compensation,
            registered_ticket,
        } => (
            active_runtime_transaction.as_ref().clone(),
            restored_runtime_transaction.as_ref().clone(),
            expected_runtime_transaction.as_ref().clone(),
            compensation.as_ref().clone(),
            registered_ticket.clone(),
        ),
        _ => return Err("authority restore outcome has no durable intent".into()),
    };
    if expected.schema_version != config::RUNTIME_COMPENSATION_SCHEMA_VERSION_V2
        || expected.state != config::RuntimeCompensationState::InProgress
        || expected_runtime_transaction != active_runtime_transaction
    {
        return Err("authority restore outcome rejected a crossed business-record boundary".into());
    }
    let Some(step_index) = expected
        .steps
        .iter()
        .position(|candidate| candidate.step == config::RuntimeCompensationStep::AuthorityRestore)
    else {
        return Err("authority restore step is not part of the durable plan".into());
    };
    if expected.steps[step_index].outcome != config::RuntimeCompensationStepState::InProgress
        || expected.steps[..step_index]
            .iter()
            .any(|candidate| !candidate.outcome.is_terminal())
        || expected.steps[step_index + 1..]
            .iter()
            .any(|candidate| candidate.outcome != config::RuntimeCompensationStepState::Pending)
    {
        return Err("authority restore outcome rejected non-canonical progress".into());
    }
    let require_restored = outcome == config::RuntimeCompensationStepState::Succeeded;
    let (next, next_runtime_transaction) = config::update_result(dir, |current| {
        if current.runtime_compensation.as_ref() != Some(&expected) {
            return Err(
                "authority restore outcome found a drifted compensation journal; preserved the current transaction"
                    .into(),
            );
        }
        let observed_runtime_transaction = if current.runtime_transaction
            == restored_runtime_transaction
        {
            restored_runtime_transaction.clone()
        } else if !require_restored && current.runtime_transaction == expected_runtime_transaction {
            expected_runtime_transaction.clone()
        } else {
            return Err(
                "authority restore outcome found a drifted business record; preserved the current transaction"
                    .into(),
            );
        };
        let mut record = expected.clone();
        record.steps[step_index].outcome = outcome;
        current.runtime_compensation = Some(record.clone());
        Ok(((record, observed_runtime_transaction), true))
    })?;
    *progress = OneClickJournalProgress::Compensating {
        active_runtime_transaction: Box::new(active_runtime_transaction),
        restored_runtime_transaction: Box::new(restored_runtime_transaction),
        expected_runtime_transaction: Box::new(next_runtime_transaction),
        compensation: Box::new(next),
        registered_ticket,
    };
    Ok(())
}

pub(in super::super) fn finish_one_click_compensation(
    dir: &Path,
    progress: &mut OneClickJournalProgress,
) -> Result<(), String> {
    let (
        active_runtime_transaction,
        restored_runtime_transaction,
        expected_runtime_transaction,
        expected,
    ) = match progress {
        OneClickJournalProgress::PreJournalAbort { .. }
        | OneClickJournalProgress::CompensationFinished { .. }
        | OneClickJournalProgress::Finalized { .. } => return Ok(()),
        OneClickJournalProgress::Journaled { .. } => {
            return Err("one-click compensation completion has no durable intent".into())
        }
        OneClickJournalProgress::Compensating {
            active_runtime_transaction,
            restored_runtime_transaction,
            expected_runtime_transaction,
            compensation,
            ..
        } => (
            active_runtime_transaction.as_ref().clone(),
            restored_runtime_transaction.as_ref().clone(),
            expected_runtime_transaction.as_ref().clone(),
            compensation.as_ref().clone(),
        ),
    };
    if expected.state != config::RuntimeCompensationState::InProgress {
        return Err("one-click compensation completion has no durable intent".into());
    }
    if expected
        .steps
        .iter()
        .any(|progress| progress.outcome == config::RuntimeCompensationStepState::InProgress)
    {
        return Err("one-click compensation completion found an unfinished step".into());
    }
    let failed_steps = expected
        .steps
        .iter()
        .filter_map(|progress| {
            (progress.outcome == config::RuntimeCompensationStepState::Failed)
                .then_some(progress.step)
        })
        .collect::<Vec<_>>();
    if failed_steps.is_empty()
        && expected
            .steps
            .iter()
            .any(|progress| !progress.outcome.is_terminal())
    {
        return Err("one-click compensation completion found a pending step".into());
    }
    let registered_ticket = progress.registered_ticket().clone();
    let next = config::update_result(dir, |current| {
        if current.runtime_compensation.as_ref() != Some(&expected)
            || current.runtime_transaction != expected_runtime_transaction
        {
            return Err(
                "one-click compensation completion found a drifted runtime journal; preserved the current transaction"
                    .into(),
            );
        }
        if failed_steps.is_empty() {
            current.runtime_compensation = None;
            Ok((None, true))
        } else {
            let mut record = expected.clone();
            record.state = config::RuntimeCompensationState::Incomplete {
                failed_steps: failed_steps.clone(),
            };
            current.runtime_compensation = Some(record.clone());
            Ok((Some(record), true))
        }
    })?;
    *progress = match next {
        Some(compensation) => OneClickJournalProgress::Compensating {
            active_runtime_transaction: Box::new(active_runtime_transaction),
            restored_runtime_transaction: Box::new(restored_runtime_transaction),
            expected_runtime_transaction: Box::new(expected_runtime_transaction),
            compensation: Box::new(compensation),
            registered_ticket,
        },
        None => OneClickJournalProgress::CompensationFinished { registered_ticket },
    };
    Ok(())
}

#[cfg(test)]
pub(in super::super) fn clear_one_click_transaction(
    dir: &Path,
    identity: &OneClickTransactionIdentity,
    progress: &mut OneClickJournalProgress,
) -> Result<(), String> {
    let expected_record = progress
        .journaled_record()
        .cloned()
        .ok_or("one-click cannot clear a journal before its first durable checkpoint")?;
    let registered_ticket = progress.registered_ticket().clone();
    config::update_result(dir, |current| {
        if !config_authority_matches(
            current,
            &identity.target_profile_id,
            identity.previous_binding.as_ref(),
        ) {
            return Err(
                "one-click clear config authority drifted; preserved the runtime transaction"
                    .into(),
            );
        }
        match current.runtime_transaction.as_ref() {
            Some(config::RuntimeTransactionRecord::V2(journal))
                if journal == &expected_record
                    && one_click_journal_matches(
                        journal,
                        identity,
                        &expected_record.transaction_id,
                    ) => {}
            _ => {
                return Err(
                    "one-click clear identity changed; preserved the runtime transaction".into(),
                )
            }
        }
        current.runtime_transaction = None;
        Ok(((), true))
    })?;
    *progress = OneClickJournalProgress::Finalized {
        record: expected_record,
        registered_ticket,
    };
    Ok(())
}

#[cfg(test)]
pub(in super::super) fn commit_runtime_binding(
    dir: &Path,
    identity: &OneClickTransactionIdentity,
    progress: &mut OneClickJournalProgress,
    binding: config::RuntimeBindingCommit,
) -> Result<(), String> {
    let expected_record = progress
        .journaled_record()
        .cloned()
        .ok_or("one-click cannot commit a binding before its first durable checkpoint")?;
    let registered_ticket = progress.registered_ticket().clone();
    config::update_result(dir, |current| {
        if !config_authority_matches(
            current,
            &identity.target_profile_id,
            identity.previous_binding.as_ref(),
        ) {
            return Err(
                "one-click binding commit config authority drifted; preserved the runtime transaction"
                    .into(),
            );
        }
        match current.runtime_transaction.as_ref() {
            Some(config::RuntimeTransactionRecord::V2(journal))
                if journal == &expected_record
                    && one_click_journal_matches(
                        journal,
                        identity,
                        &expected_record.transaction_id,
                    ) => {}
            _ => {
                return Err(
                    "one-click binding commit identity changed; preserved the runtime transaction"
                        .into(),
                )
            }
        }
        current.runtime_binding = Some(binding.clone());
        current.runtime_transaction = None;
        Ok(((), true))
    })?;
    *progress = OneClickJournalProgress::Finalized {
        record: expected_record,
        registered_ticket,
    };
    Ok(())
}

pub(in super::super) fn begin_one_click_finalize(
    dir: &Path,
    identity: &OneClickTransactionIdentity,
    progress: &mut OneClickJournalProgress,
    action: config::RuntimeFinalizeAction,
) -> Result<(), String> {
    let expected = progress
        .journaled_record()
        .cloned()
        .ok_or("one-click cannot begin finalize before its first durable checkpoint")?;
    let registered_ticket = progress.registered_ticket().clone();
    let next = config::update_result(dir, |current| {
        if !config_authority_matches(
            current,
            &identity.target_profile_id,
            identity.previous_binding.as_ref(),
        ) {
            return Err(
                "one-click finalize intent config authority drifted; preserved the runtime transaction"
                    .into(),
            );
        }
        match current.runtime_transaction.as_ref() {
            Some(config::RuntimeTransactionRecord::V2(journal))
                if journal == &expected
                    && one_click_journal_matches(journal, identity, &expected.transaction_id) => {}
            _ => {
                return Err(
                    "one-click finalize intent identity changed; preserved the runtime transaction"
                        .into(),
                )
            }
        }
        let mut next = expected.clone();
        next.finalize = config::RuntimeFinalizeState::Intent {
            action: action.clone(),
        };
        current.runtime_transaction = Some(config::RuntimeTransactionRecord::V2(next.clone()));
        Ok((next, true))
    })?;
    *progress = OneClickJournalProgress::Journaled {
        record: next,
        registered_ticket,
    };
    Ok(())
}

pub(in super::super) fn complete_one_click_finalize(
    dir: &Path,
    progress: &mut OneClickJournalProgress,
) -> Result<(), String> {
    let expected = progress
        .journaled_record()
        .cloned()
        .ok_or("one-click cannot complete finalize without a durable intent")?;
    let action = match &expected.finalize {
        config::RuntimeFinalizeState::Intent { action } => action.clone(),
        config::RuntimeFinalizeState::NotStarted => {
            return Err("one-click finalize intent is missing".into())
        }
    };
    let registered_ticket = progress.registered_ticket().clone();
    #[cfg(test)]
    if SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .one_click_fail_finalize_completion
        .as_deref()
        == Some(dir)
    {
        return Err("test-only one-click finalize config commit failure".into());
    }
    config::update_result(dir, |current| {
        if !config_authority_matches(
            current,
            &expected.target_profile_id,
            expected.previous_binding.as_ref(),
        ) || current.runtime_transaction.as_ref()
            != Some(&config::RuntimeTransactionRecord::V2(expected.clone()))
        {
            return Err(
                "one-click finalize completion identity changed; preserved the runtime transaction"
                    .into(),
            );
        }
        if let config::RuntimeFinalizeAction::CommitBinding { binding, .. } = &action {
            current.runtime_binding = Some(binding.clone());
        }
        current.runtime_transaction = None;
        Ok(((), true))
    })?;
    *progress = OneClickJournalProgress::Finalized {
        record: expected,
        registered_ticket,
    };
    Ok(())
}
