//! Shared process-local ownership for exact Science stops inside durable transactions.

use crate::runtime::science::{
    ScienceRuntimeIdentity, ScienceStopFailure, ScienceStopOutcome, ScienceStopRequest,
};
use crate::{lifecycle, lock, AppState, SharedAppState};

#[derive(Clone, Copy, Debug)]
pub(super) enum TransactionScienceStopBoundary {
    ColdPriorStop,
    ManagedDbRestart,
    HistoryRecoveryPriorStop,
    LiveCompensationCleanup,
    CompensationReplayCleanup,
}

impl TransactionScienceStopBoundary {
    fn code(self) -> &'static str {
        match self {
            Self::ColdPriorStop => "cold_prior_stop",
            Self::ManagedDbRestart => "managed_db_restart",
            Self::HistoryRecoveryPriorStop => "history_recovery_prior_stop",
            Self::LiveCompensationCleanup => "live_compensation_cleanup",
            Self::CompensationReplayCleanup => "compensation_replay_cleanup",
        }
    }

    fn permits_untracked_durable_owner(self) -> bool {
        matches!(
            self,
            Self::LiveCompensationCleanup | Self::CompensationReplayCleanup
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TransactionScienceStopTarget<'a> {
    boundary: TransactionScienceStopBoundary,
    expected_runtime: &'a ScienceRuntimeIdentity,
    expected_port: u16,
}

impl<'a> TransactionScienceStopTarget<'a> {
    pub(super) fn new(
        boundary: TransactionScienceStopBoundary,
        expected_runtime: &'a ScienceRuntimeIdentity,
        expected_port: u16,
    ) -> Self {
        Self {
            boundary,
            expected_runtime,
            expected_port,
        }
    }
}

#[derive(Clone)]
struct TransactionScienceStopOwner {
    generation: u64,
    runtime: Option<ScienceRuntimeIdentity>,
    confirmed_stopped: Option<ScienceRuntimeIdentity>,
    sandbox_child_pid: Option<u32>,
    sandbox_port: u16,
    sandbox_url: Option<String>,
}

impl TransactionScienceStopOwner {
    fn claim(state: &AppState, generation: u64) -> Self {
        Self {
            generation,
            runtime: state.science_runtime.clone(),
            confirmed_stopped: state.science_confirmed_stopped.clone(),
            sandbox_child_pid: state.sandbox.as_ref().map(std::process::Child::id),
            sandbox_port: state.sandbox_port,
            sandbox_url: state.sandbox_url.clone(),
        }
    }

    fn can_stop(
        &self,
        boundary: TransactionScienceStopBoundary,
        expected_runtime: &ScienceRuntimeIdentity,
        expected_port: u16,
    ) -> bool {
        match self.runtime.as_ref() {
            Some(runtime) => runtime == expected_runtime && self.sandbox_port == expected_port,
            None if boundary.permits_untracked_durable_owner() => {
                self.sandbox_child_pid.is_none() && self.sandbox_url.is_none()
            }
            None => false,
        }
    }

    fn still_owns(&self, state: &AppState, current_generation: u64) -> bool {
        self.generation == current_generation
            && self.runtime == state.science_runtime
            && self.confirmed_stopped == state.science_confirmed_stopped
            && self.sandbox_child_pid == state.sandbox.as_ref().map(std::process::Child::id)
            && self.sandbox_port == state.sandbox_port
            && self.sandbox_url == state.sandbox_url
    }
}

/// Execute one exact Science stop inside a durable runtime transaction.
///
/// The caller commits durable intent first. AppState protects only owner
/// snapshots and CAS publication. Request probing and stop/TERM/KILL/wait are
/// external work and therefore execute without the AppState mutex.
pub(super) fn execute_transaction_science_stop_with<Claim, Execute, AfterPublish>(
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    target: TransactionScienceStopTarget<'_>,
    claim_exact_request: Claim,
    execute: Execute,
    after_publish: AfterPublish,
) -> ScienceStopOutcome
where
    Claim: FnOnce() -> Result<ScienceStopRequest, ScienceStopFailure>,
    Execute: FnOnce(ScienceStopRequest) -> (ScienceStopOutcome, bool),
    AfterPublish: FnOnce(&mut AppState, Option<&ScienceRuntimeIdentity>),
{
    let TransactionScienceStopTarget {
        boundary,
        expected_runtime,
        expected_port,
    } = target;
    let owner = {
        let current = lock(state);
        TransactionScienceStopOwner::claim(&current, lifecycle.current_generation())
    };

    let request = if owner.can_stop(boundary, expected_runtime, expected_port) {
        claim_exact_request()
    } else {
        Err(ScienceStopFailure::request_rejected(format!(
            "{} Science stop claim 时 process-local owner 已变化；未执行停止。",
            boundary.code()
        )))
    };

    let request = request.and_then(|request| {
        let current = lock(state);
        if owner.still_owns(&current, lifecycle.current_generation()) {
            Ok(request)
        } else {
            Err(ScienceStopFailure::request_rejected(format!(
                "{} Science stop probe 完成时 process-local owner 已变化；未执行停止。",
                boundary.code()
            )))
        }
    });

    let execution = request.map(|request| {
        let (outcome, clear_tracking) = execute(request);
        (
            outcome.and_then(|verified| verified.require_exact_stop_of(expected_runtime)),
            clear_tracking,
        )
    });

    let (outcome, mut stopped_child) = {
        let mut current = lock(state);
        match execution {
            Err(error) => (Err(error), None),
            Ok((_outcome, _clear_tracking))
                if !owner.still_owns(&current, lifecycle.current_generation()) =>
            {
                (
                    Err(ScienceStopFailure::outcome_publication_failure(format!(
                        "{} Science stop 完成时 process-local owner 已变化；已保留 replacement runtime。",
                        boundary.code()
                    ))),
                    None,
                )
            }
            Ok((outcome, clear_tracking)) => {
                let stopped_child = if clear_tracking {
                    current.sandbox_url = None;
                    current.sandbox.take()
                } else {
                    None
                };
                let confirmed_runtime = match outcome.as_ref() {
                    Ok(verified) => verified.confirmed_runtime(),
                    Err(error) => error.confirmed_runtime(),
                }
                .filter(|runtime| *runtime == expected_runtime)
                .cloned();
                if let Some(confirmed_runtime) = confirmed_runtime.as_ref() {
                    current.science_runtime = None;
                    current.science_confirmed_stopped = Some(confirmed_runtime.clone());
                }
                after_publish(&mut current, confirmed_runtime.as_ref());
                (outcome, stopped_child)
            }
        }
    };
    crate::runtime::system::kill_child(&mut stopped_child);
    outcome
}
