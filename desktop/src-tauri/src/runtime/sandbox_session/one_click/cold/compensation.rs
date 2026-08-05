use super::*;

#[derive(Debug)]
pub(in super::super::super) enum CompensationCause {
    ScienceCleanup { safe_detail: String },
    SshCleanup,
    AuthorityRestore,
    PriorScienceRestart(ManagedScienceRestartError),
    SnapshotCleanup(AuthorityCleanupFailure),
}

impl CompensationCause {
    fn render_diagnostic(&self) -> String {
        match self {
            Self::ScienceCleanup { safe_detail } => format!(
                "compensation_science_cleanup_failed；compensation_restore_blocked_science_candidate；{safe_detail}"
            ),
            Self::SshCleanup => "compensation_ssh_cleanup_failed".into(),
            Self::AuthorityRestore => "compensation_restore_failed".into(),
            Self::PriorScienceRestart(error) => match error.diagnostic {
                #[cfg(test)]
                PriorScienceRestartDiagnostic::TestPostSpawnValidationFailed => {
                    "test-only prior Science post-spawn validation failure".into()
                }
                PriorScienceRestartDiagnostic::Failed => {
                    "compensation_prior_science_restart_failed".into()
                }
            },
            Self::SnapshotCleanup(error) if error.cleanup_requirement().is_some() => {
                error.to_string()
            }
            Self::SnapshotCleanup(_) => "compensation_snapshot_register_failed".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super::super) enum CompensationSkipCause {
    NoScienceCandidate,
    NoPriorScience,
    CrossRuntimeEnvironment,
    BlockedByScienceCleanup,
    BlockedByAuthorityRestore,
    SnapshotPreserved,
}

#[derive(Debug)]
pub(in super::super::super) enum CompensationStepOutcome {
    Succeeded,
    Skipped(CompensationSkipCause),
    Failed(CompensationCause),
}

impl CompensationStepOutcome {
    fn succeeded(&self) -> bool {
        matches!(self, Self::Succeeded)
    }

    fn completed_or_not_required(&self) -> bool {
        matches!(
            self,
            Self::Succeeded
                | Self::Skipped(CompensationSkipCause::NoScienceCandidate)
                | Self::Skipped(CompensationSkipCause::NoPriorScience)
                | Self::Skipped(CompensationSkipCause::CrossRuntimeEnvironment)
        )
    }

    fn cause(&self) -> Option<&CompensationCause> {
        match self {
            Self::Failed(cause) => Some(cause),
            Self::Succeeded | Self::Skipped(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super::super) enum CompensationEnvironment {
    Quiescent,
    Candidate,
    CrossRuntime,
}

impl CompensationEnvironment {
    fn from_launch(environment: ScienceEnvironmentExposure, cross_runtime: bool) -> Self {
        if cross_runtime {
            Self::CrossRuntime
        } else {
            match environment {
                ScienceEnvironmentExposure::NotExposed => Self::Quiescent,
                ScienceEnvironmentExposure::Exposed | ScienceEnvironmentExposure::Uncertain => {
                    Self::Candidate
                }
            }
        }
    }

    fn is_uncertain(self) -> bool {
        self != Self::Quiescent
    }

    fn append_diagnostics(self, diagnostics: &mut Vec<String>) {
        if self.is_uncertain() {
            diagnostics.push("environment_uncertain".into());
        }
        if self == Self::CrossRuntime {
            diagnostics.push("newer_runtime_required".into());
        }
    }
}

#[derive(Debug)]
pub(in super::super::super) struct CompensationOutcome {
    pub(in super::super::super) science_cleanup: CompensationStepOutcome,
    pub(in super::super::super) ssh_cleanup: CompensationStepOutcome,
    pub(in super::super::super) authority_restore: CompensationStepOutcome,
    pub(in super::super::super) prior_science_restart: CompensationStepOutcome,
    pub(in super::super::super) snapshot_cleanup: CompensationStepOutcome,
    pub(in super::super::super) environment: CompensationEnvironment,
}

impl CompensationOutcome {
    fn blocked_by_science_cleanup(
        safe_detail: String,
        environment: CompensationEnvironment,
    ) -> Self {
        Self {
            science_cleanup: CompensationStepOutcome::Failed(CompensationCause::ScienceCleanup {
                safe_detail,
            }),
            ssh_cleanup: CompensationStepOutcome::Skipped(
                CompensationSkipCause::BlockedByScienceCleanup,
            ),
            authority_restore: CompensationStepOutcome::Skipped(
                CompensationSkipCause::BlockedByScienceCleanup,
            ),
            prior_science_restart: CompensationStepOutcome::Skipped(
                CompensationSkipCause::BlockedByScienceCleanup,
            ),
            snapshot_cleanup: CompensationStepOutcome::Skipped(
                CompensationSkipCause::SnapshotPreserved,
            ),
            environment,
        }
    }

    pub(in super::super::super) fn authorities_restored(&self) -> bool {
        self.science_cleanup.completed_or_not_required()
            && self.ssh_cleanup.succeeded()
            && self.authority_restore.succeeded()
            && self.prior_science_restart.completed_or_not_required()
    }

    pub(in super::super::super) fn prior_science_restored(&self) -> bool {
        self.authorities_restored() && self.prior_science_restart.succeeded()
    }

    fn cleanup_required(&self) -> bool {
        matches!(
            &self.snapshot_cleanup,
            CompensationStepOutcome::Failed(CompensationCause::SnapshotCleanup(error))
                if error.cleanup_requirement().is_some()
        ) || matches!(
            self.science_cleanup,
            CompensationStepOutcome::Failed(CompensationCause::ScienceCleanup { .. })
        )
    }

    pub(in super::super::super) fn projected_recovery(&self) -> ProjectedRecovery {
        if self.cleanup_required() && self.environment.is_uncertain() {
            ProjectedRecovery::cleanup_required_uncertain()
        } else if self.cleanup_required() {
            ProjectedRecovery::CLEANUP_REQUIRED
        } else if self.environment.is_uncertain() {
            ProjectedRecovery::ENVIRONMENT_UNCERTAIN
        } else if self.authorities_restored() {
            ProjectedRecovery::NOT_NEEDED
        } else {
            ProjectedRecovery::DEGRADED
        }
    }

    fn render_failure_message(&self, primary: &str) -> String {
        let mut diagnostics = [
            &self.science_cleanup,
            &self.ssh_cleanup,
            &self.authority_restore,
        ]
        .into_iter()
        .filter_map(CompensationStepOutcome::cause)
        .map(CompensationCause::render_diagnostic)
        .collect::<Vec<_>>();
        self.environment.append_diagnostics(&mut diagnostics);
        diagnostics.extend(
            [&self.prior_science_restart, &self.snapshot_cleanup]
                .into_iter()
                .filter_map(CompensationStepOutcome::cause)
                .map(CompensationCause::render_diagnostic),
        );
        if diagnostics.is_empty() {
            primary.to_string()
        } else {
            format!("{primary}；{}", diagnostics.join("; "))
        }
    }
}

#[allow(clippy::result_large_err, clippy::too_many_arguments)]
pub(in super::super) fn compensate_one_click_failure<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
    dir: &Path,
    trace: &OperationTrace,
    authority_transaction: &mut AuthorityTransaction,
    journal_progress: &OneClickJournalProgress,
    prior_science: Option<&PriorScienceContext>,
    failure: OneClickFailure,
    mut reconcile_disposition: Option<&mut PriorScienceDisposition>,
) -> Result<Value, TypedOneClickFailure> {
    let original_kind = failure.typed.kind();
    if let OneClickJournalProgress::PreJournalAbort { registered_ticket } = journal_progress {
        let in_memory_ticket = authority_transaction.registered_snapshot_ticket();
        if in_memory_ticket.as_ref() != Ok(registered_ticket) {
            authority_transaction.preserve_recovery();
            trace.finish("error=pre_journal_abort_ticket_unverified");
            return Err(TypedOneClickFailure::new(
                original_kind,
                "首个 one-click V2 checkpoint 失败，且同进程 registered snapshot ticket 无法重新验证；已保留 ActiveRecovery 并要求人工恢复。",
            )
            .with_recovery(ProjectedRecovery::MANUAL_RECOVERY_REQUIRED));
        }
    }
    let cross_runtime_environment = failure.rollback.launch_environment.may_be_exposed()
        && prior_science.is_some_and(|prior| prior.runtime != failure.rollback.launch_runtime);
    let environment = CompensationEnvironment::from_launch(
        failure.rollback.launch_environment,
        cross_runtime_environment,
    );
    let science_cleanup_required = failure.rollback.launch_environment.may_be_exposed()
        || failure.rollback.launch_token.is_some();
    let cleanup = if failure.rollback.candidate_stop_proof
        == ManagedScienceCandidateStopProof::Unproven
    {
        Err("code=science_candidate_stop_unproven".into())
    } else if !failure.rollback.launch_environment.may_be_exposed()
        && failure.rollback.launch_token.is_none()
    {
        Ok(())
    } else if failure.rollback.launch_confirmed_stopped {
        let receipt = dir.join("science-managed-launch.v1.json");
        if proc::loopback_port_in_use(
            failure.rollback.sandbox_port,
            operation::LOCAL_HEALTH_TIMEOUT_MS,
        ) || failure
            .rollback
            .launch_token
            .as_ref()
            .is_some_and(ScienceHostAdapter::receipt_process_is_alive)
            || receipt.exists()
        {
            Err("DB recovery restart 前已停止的 Science 身份重新出现；拒绝恢复 authority".into())
        } else {
            Ok(())
        }
    } else {
        let mut current = lock(state);
        let AppState {
            sandbox,
            sandbox_url,
            ..
        } = &mut *current;
        let result = ScienceHostAdapter::stop(
            app,
            sandbox,
            sandbox_url,
            failure
                .rollback
                .launch_token
                .as_ref()
                .map(|token| {
                    ScienceStopRequest::exact(
                        &failure.rollback.launch_runtime,
                        ScienceStopOwnershipReceipt::from_managed_launch(token),
                    )
                })
                .unwrap_or_else(|| {
                    ScienceStopRequest::recover(Some(&failure.rollback.launch_runtime))
                }),
        );
        let result = result
            .and_then(|verified| verified.require_exact_stop_of(&failure.rollback.launch_runtime));
        if let Ok(verified) = result.as_ref() {
            current.science_runtime = None;
            current.science_confirmed_stopped = verified.confirmed_runtime().cloned();
        }
        result.map(|_| ()).map_err(|error| error.to_string())
    };
    if let Err(cleanup_error) = cleanup.as_ref() {
        let outcome =
            CompensationOutcome::blocked_by_science_cleanup(cleanup_error.to_string(), environment);
        if outcome.environment.is_uncertain() {
            if let Some(disposition) = reconcile_disposition.as_deref_mut() {
                *disposition = PriorScienceDisposition::EnvironmentUncertain;
            }
        }
        authority_transaction.preserve_recovery();
        trace.finish("error=compensation_restore_blocked_science_cleanup_unproven");
        let cleanup_failure = cleanup_required_error(
            AuthorityCleanupPhase::Cleanup,
            &outcome.render_failure_message(failure.message()),
            authority_transaction.recovery_path(),
            "science_candidate_stop_unproven",
        );
        let phase = cleanup_failure.phase();
        return Err(
            TypedOneClickFailure::new(original_kind, cleanup_failure.to_string())
                .with_recovery(outcome.projected_recovery())
                .with_safe_cause(phase.cause_code(), "authority cleanup typed failure"),
        );
    }
    let ssh_cleanup = match failure.rollback.ssh_stub_transaction.as_ref() {
        Some(transaction) => transaction.compensate(&sandbox_home()),
        None => crate::runtime::settings::remove_managed_sandbox_ssh_stub(&sandbox_home()),
    };
    let runtime_transaction_restore = journal_progress.restore_expectation();
    let rollback = authority_transaction.restore(
        app,
        dir,
        state,
        lifecycle,
        auth_proof,
        failure.rollback.proxy_action,
        &runtime_transaction_restore,
    );
    let prior_restart = if rollback.is_ok() && !cross_runtime_environment {
        match prior_science {
            Some(prior) => match restart_prior_science(app, state, lifecycle, auth_proof, prior) {
                Ok(()) => CompensationStepOutcome::Succeeded,
                Err(error) => {
                    CompensationStepOutcome::Failed(CompensationCause::PriorScienceRestart(error))
                }
            },
            None => CompensationStepOutcome::Skipped(CompensationSkipCause::NoPriorScience),
        }
    } else if cross_runtime_environment {
        CompensationStepOutcome::Skipped(CompensationSkipCause::CrossRuntimeEnvironment)
    } else {
        CompensationStepOutcome::Skipped(CompensationSkipCause::BlockedByAuthorityRestore)
    };
    let mut outcome = CompensationOutcome {
        science_cleanup: if science_cleanup_required {
            CompensationStepOutcome::Succeeded
        } else {
            CompensationStepOutcome::Skipped(CompensationSkipCause::NoScienceCandidate)
        },
        ssh_cleanup: match ssh_cleanup {
            Ok(_) => CompensationStepOutcome::Succeeded,
            Err(_) => CompensationStepOutcome::Failed(CompensationCause::SshCleanup),
        },
        authority_restore: match rollback {
            Ok(()) => CompensationStepOutcome::Succeeded,
            Err(_) => CompensationStepOutcome::Failed(CompensationCause::AuthorityRestore),
        },
        prior_science_restart: prior_restart,
        snapshot_cleanup: CompensationStepOutcome::Skipped(
            CompensationSkipCause::SnapshotPreserved,
        ),
        environment,
    };
    let authorities_restored = outcome.authorities_restored();
    let prior_science_restored = outcome.prior_science_restored();
    if let Some(disposition) = reconcile_disposition {
        if outcome.environment.is_uncertain() {
            *disposition = PriorScienceDisposition::EnvironmentUncertain;
        } else if prior_science_restored {
            *disposition = PriorScienceDisposition::Restored;
        }
    }
    outcome.snapshot_cleanup = if authorities_restored {
        match authority_transaction.cleanup_when_expendable() {
            Ok(_) => CompensationStepOutcome::Succeeded,
            Err(error) => {
                CompensationStepOutcome::Failed(CompensationCause::SnapshotCleanup(error))
            }
        }
    } else {
        authority_transaction.preserve_recovery();
        CompensationStepOutcome::Skipped(CompensationSkipCause::SnapshotPreserved)
    };
    trace.finish(
        if authorities_restored && outcome.environment.is_uncertain() {
            "error=one_click_transaction_compensated environment=uncertain"
        } else if authorities_restored {
            "error=one_click_transaction_compensated environment=not_exposed"
        } else {
            "error=one_click_compensation_incomplete"
        },
    );
    let message = outcome.render_failure_message(failure.message());
    let recovery = outcome.projected_recovery();
    Err(TypedOneClickFailure::new(original_kind, message).with_recovery(recovery))
}
