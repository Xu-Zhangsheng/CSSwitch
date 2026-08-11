#[test]
fn one_click_snapshot_has_one_commit_and_one_failure_compensation_funnel() {
    use super::super::one_click::{
        decide_one_click_entry_recovery, pending_cleanup_requires_recapture, CompensationCause,
        CompensationEnvironment, CompensationOutcome, CompensationSkipCause,
        CompensationStepOutcome, OneClickEntryRecoveryDecision,
    };
    use super::super::pending_cleanup::PendingCleanupRetryOutcome;
    use crate::runtime::failure::ProjectedRecovery;
    use syn::visit::{self, Visit};
    use syn::{Expr, ExprCall, ExprMethodCall, Item, ItemFn, Pat, Stmt};

    assert_eq!(
        decide_one_click_entry_recovery(true, false, false, false),
        OneClickEntryRecoveryDecision::ReplayFinalize
    );
    assert_eq!(
        decide_one_click_entry_recovery(false, true, false, false),
        OneClickEntryRecoveryDecision::ReplayFinalizeCleanup
    );
    assert_eq!(
        decide_one_click_entry_recovery(false, true, true, false),
        OneClickEntryRecoveryDecision::RecoverGateway
    );
    assert_eq!(
        decide_one_click_entry_recovery(false, true, true, true),
        OneClickEntryRecoveryDecision::Route
    );
    assert!(pending_cleanup_requires_recapture(
        PendingCleanupRetryOutcome::Cleared
    ));
    assert!(!pending_cleanup_requires_recapture(
        PendingCleanupRetryOutcome::NotNeeded
    ));

    fn top_level<'a>(file: &'a syn::File, name: &str) -> Option<&'a ItemFn> {
        file.items.iter().find_map(|item| match item {
            Item::Fn(function) if function.sig.ident == name => Some(function),
            _ => None,
        })
    }

    fn local_name(local: &syn::Local) -> Option<&syn::Ident> {
        match &local.pat {
            Pat::Ident(ident) => Some(&ident.ident),
            Pat::Type(typed) => match &*typed.pat {
                Pat::Ident(ident) => Some(&ident.ident),
                _ => None,
            },
            _ => None,
        }
    }

    fn result_arm_name(pattern: &Pat) -> Option<&syn::Ident> {
        match pattern {
            Pat::TupleStruct(tuple) => tuple.path.segments.last().map(|segment| &segment.ident),
            Pat::Struct(structure) => structure.path.segments.last().map(|segment| &segment.ident),
            Pat::Path(path) => path.path.segments.last().map(|segment| &segment.ident),
            Pat::Ident(ident) => ident
                .subpat
                .as_ref()
                .and_then(|(_, pattern)| result_arm_name(pattern)),
            _ => None,
        }
    }

    fn peel_expr(mut expression: &Expr) -> &Expr {
        loop {
            expression = match expression {
                Expr::Group(group) => &group.expr,
                Expr::Paren(paren) => &paren.expr,
                _ => return expression,
            };
        }
    }

    fn direct_call_name(expression: &Expr) -> Option<&syn::Ident> {
        let Expr::Call(call) = peel_expr(expression) else {
            return None;
        };
        let Expr::Path(path) = peel_expr(&call.func) else {
            return None;
        };
        path.path.segments.last().map(|segment| &segment.ident)
    }

    fn success_tail_is_infallible(expression: &Expr) -> bool {
        match peel_expr(expression) {
            Expr::Path(_) => true,
            Expr::Call(call)
                if direct_call_name(expression).is_some_and(|name| name == "Ok")
                    && call.args.len() == 1 =>
            {
                matches!(peel_expr(call.args.first().unwrap()), Expr::Path(_))
            }
            _ => false,
        }
    }

    #[derive(Default)]
    struct FlowFacts {
        calls: Vec<String>,
        methods: Vec<String>,
        tries: usize,
        closures: usize,
    }

    impl<'ast> Visit<'ast> for FlowFacts {
        fn visit_expr_call(&mut self, expression: &'ast ExprCall) {
            if let Expr::Path(path) = &*expression.func {
                if let Some(segment) = path.path.segments.last() {
                    self.calls.push(segment.ident.to_string());
                }
            }
            visit::visit_expr_call(self, expression);
        }

        fn visit_expr_method_call(&mut self, expression: &'ast ExprMethodCall) {
            self.methods.push(expression.method.to_string());
            visit::visit_expr_method_call(self, expression);
        }

        fn visit_expr_try(&mut self, expression: &'ast syn::ExprTry) {
            self.tries += 1;
            visit::visit_expr_try(self, expression);
        }

        fn visit_expr_closure(&mut self, expression: &'ast syn::ExprClosure) {
            self.closures += 1;
            visit::visit_expr_closure(self, expression);
        }
    }

    #[derive(Debug, Default)]
    struct OuterFacts {
        calls: Vec<String>,
        methods: Vec<String>,
        tries: usize,
        returns: usize,
        assignments: usize,
        macros: usize,
        closures: usize,
    }

    impl<'ast> Visit<'ast> for OuterFacts {
        fn visit_expr_call(&mut self, expression: &'ast ExprCall) {
            if let Expr::Path(path) = &*expression.func {
                if let Some(segment) = path.path.segments.last() {
                    self.calls.push(segment.ident.to_string());
                }
            }
            visit::visit_expr_call(self, expression);
        }

        fn visit_expr_method_call(&mut self, expression: &'ast ExprMethodCall) {
            self.methods.push(expression.method.to_string());
            visit::visit_expr_method_call(self, expression);
        }

        fn visit_expr_try(&mut self, expression: &'ast syn::ExprTry) {
            self.tries += 1;
            visit::visit_expr_try(self, expression);
        }

        fn visit_expr_return(&mut self, expression: &'ast syn::ExprReturn) {
            self.returns += 1;
            visit::visit_expr_return(self, expression);
        }

        fn visit_expr_assign(&mut self, expression: &'ast syn::ExprAssign) {
            self.assignments += 1;
            visit::visit_expr_assign(self, expression);
        }

        fn visit_macro(&mut self, expression: &'ast syn::Macro) {
            self.macros += 1;
            visit::visit_macro(self, expression);
        }

        fn visit_expr_closure(&mut self, _expression: &'ast syn::ExprClosure) {
            self.closures += 1;
        }
        fn visit_item_fn(&mut self, _function: &'ast ItemFn) {}
    }

    #[derive(Default)]
    struct TransactionLocalCount(usize);

    impl<'ast> Visit<'ast> for TransactionLocalCount {
        fn visit_local(&mut self, local: &'ast syn::Local) {
            if local_name(local).is_some_and(|name| name == "transaction_result") {
                self.0 += 1;
            }
            visit::visit_local(self, local);
        }
    }

    let source = include_str!("../one_click.rs");
    let transaction_stop_source = include_str!("../transaction_science_stop.rs");
    let recovery_source = include_str!("../recovery.rs");
    let failure_source = include_str!("../../failure.rs");
    let command_projection_source = include_str!("../../../commands/runtime/one_click.rs");
    let pending_cleanup_source = include_str!("../pending_cleanup.rs");
    let gateway_recovery_source = include_str!("../../proxy_lifecycle/recovery.rs");
    let cold_source = include_str!("../one_click/cold.rs");
    let compensation_source = include_str!("../one_click/cold/compensation.rs");
    let science_phase_source = include_str!("../one_click/cold/science_phase.rs");
    let healthy_reopen_source = include_str!("../one_click/healthy_reopen.rs");
    let profile_reconcile_source = include_str!("../../profile_switch.rs");
    let auto_boot_source = include_str!("../../../lib.rs");
    let config_source = include_str!("../../../config.rs");
    let profile_source = include_str!("../../profile.rs");
    let finalize_consumer_source = include_str!("../../finalize_consumer.rs");
    let history_recovery_source = include_str!("../history_recovery.rs");
    let lifecycle_command_source = include_str!("../../../commands/runtime/lifecycle.rs");
    let codex_command_source = include_str!("../../../commands/codex.rs");

    let failure_production = failure_source
        .split("#[cfg(test)]\nmod tests")
        .next()
        .expect("typed failure production boundary must remain discoverable");
    assert!(
        !failure_production.contains("recovery_from_diagnostic_codes")
            && !failure_production.contains("impl std::ops::Deref"),
        "typed runtime failure must not recover classification or string methods from message text"
    );
    for (name, typed_error_source, error_type) in [
        (
            "authority cleanup",
            pending_cleanup_source,
            "AuthorityCleanupFailure",
        ),
        (
            "interrupted Gateway recovery",
            gateway_recovery_source,
            "InterruptedGatewayRecoveryError",
        ),
    ] {
        assert!(
            !typed_error_source.contains(&format!("impl std::ops::Deref for {error_type}")),
            "{name} error must not expose implicit string contains compatibility"
        );
    }
    let gateway_error_constructor = gateway_recovery_source
        .split("impl InterruptedGatewayRecoveryError")
        .nth(1)
        .and_then(|tail| tail.split("impl std::fmt::Display").next())
        .expect("interrupted Gateway error constructor must remain discoverable");
    assert!(
        gateway_recovery_source.contains("recovery: InterruptedGatewayRecoveryDisposition")
            && gateway_error_constructor.contains(
                "InterruptedGatewayRecoveryErrorKind::AuthoritySnapshot => {\n                InterruptedGatewayRecoveryDisposition::ManualRecoveryRequired\n            }"
            )
            && gateway_error_constructor.contains(
                "InterruptedGatewayRecoveryErrorKind::GatewayStart\n            | InterruptedGatewayRecoveryErrorKind::NotManaged\n            | InterruptedGatewayRecoveryErrorKind::StopUnknown(_) => {\n                InterruptedGatewayRecoveryDisposition::Degraded\n            }"
            ),
        "interrupted Gateway error constructor must map AuthoritySnapshot to manual recovery and all other kinds to degraded"
    );
    let gateway_projection = source
        .split("fn typed_interrupted_gateway_recovery_error")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(super) enum TransactionScienceStopBoundary")
                .next()
        })
        .expect("interrupted Gateway runtime projection must remain discoverable");
    assert!(
        gateway_projection.contains("error.kind()")
            && gateway_projection.contains("error.recovery()")
            && gateway_projection.contains(
                "InterruptedGatewayRecoveryDisposition::Degraded => ProjectedRecovery::DEGRADED"
            )
            && gateway_projection
                .contains("InterruptedGatewayRecoveryDisposition::ManualRecoveryRequired")
            && gateway_projection.contains("ProjectedRecovery::MANUAL_RECOVERY_REQUIRED")
            && gateway_projection.contains(".with_recovery(recovery)")
            && !gateway_projection.contains(".contains("),
        "interrupted Gateway runtime projection must map kind and recovery from typed fields"
    );
    let gateway_recovery_writer = gateway_recovery_source
        .split("fn interrupted_gateway_recovery_record")
        .nth(1)
        .and_then(|tail| {
            tail.split("/// Consume an interrupted profile-switch journal")
                .next()
        })
        .expect("interrupted Gateway typed writer must remain discoverable");
    let gateway_recovery_finish = gateway_recovery_source
        .split("fn finish_interrupted_gateway_recovery")
        .nth(1)
        .expect("interrupted Gateway outcome writer must remain discoverable");
    assert!(
        gateway_recovery_writer.contains("RuntimeTransactionRecord::V2")
            && gateway_recovery_writer.contains(
                "current.runtime_transaction.as_ref() != Some(expected)"
            )
            && gateway_recovery_writer.contains("RuntimeTransactionOperation::ProfileSwitch")
            && gateway_recovery_writer.contains("RuntimeTransactionPhase::RecoverInterruptedGateway")
            && gateway_recovery_writer.contains("RuntimeGatewayStopOutcome::AbsentAfterAttempt")
            && gateway_recovery_source.contains("interrupted_gateway_recovery_is_complete")
            && gateway_recovery_finish.contains("RuntimeGatewayStopOutcome::Pending")
            && gateway_recovery_finish.contains("RuntimeGatewayStopOutcome::Stopped")
            && gateway_recovery_finish.contains("RuntimeGatewayStopOutcome::NotManaged")
            && gateway_recovery_finish.contains("RuntimeGatewayStopOutcome::SignalFailed")
            && gateway_recovery_finish.contains("RuntimeGatewayStopOutcome::ExitUnconfirmed")
            && gateway_recovery_finish.contains(
                "publish_interrupted_gateway_recovery_record(dir, &pending, outcome)"
            )
            && !gateway_recovery_source.contains(".as_v1_mut()")
            && !gateway_recovery_source.contains("当前 R2-A 兼容层"),
        "interrupted Gateway recovery must publish typed V2 intent/outcomes with complete-record CAS and never write a V1 string stage"
    );
    let one_click_progress = source
        .split("pub(super) enum OneClickJournalProgress")
        .nth(1)
        .and_then(|tail| tail.split("pub(super) fn one_click_phase_exposure").next())
        .expect("one-click journal progress must remain discoverable");
    let one_click_writer = source
        .split("pub(super) fn write_one_click_checkpoint")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(super) fn validate_interrupted_science_transaction_entry")
                .next()
        })
        .expect("one-click V2 writer must remain discoverable");
    let compensation_journal = source
        .split("pub(super) fn begin_one_click_compensation(")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(super) fn validate_interrupted_science_transaction_entry")
                .next()
        })
        .expect("durable one-click compensation journal writers must remain discoverable");
    let one_click_terminal_writers = source
        .split("pub(super) fn clear_one_click_transaction")
        .nth(1)
        .and_then(|tail| tail.split("fn history_recovery_choices").next())
        .expect("one-click terminal journal writers must remain discoverable");
    let one_click_restore = recovery_source
        .split("pub(super) fn restore_with_gateway")
        .nth(2)
        .expect("one-click authority restore must remain discoverable");
    let restore_guard = one_click_restore
        .find("let authority_matches = match runtime_transaction")
        .expect("one-click compensation must preflight the current journal");
    let first_restore_effect = one_click_restore
        .find(".stop_proxy()")
        .expect("one-click compensation runtime restore must remain discoverable");
    let confirmed_gateway_stop = one_click_restore
        .find(".require_stopped(\"one-click compensation 恢复前无法安全停止 Gateway\")")
        .expect("one-click compensation must fail closed on uncertain Gateway stop");
    assert!(
        one_click_progress.contains("record: config::RuntimeTransactionV2")
            && one_click_progress.contains("Self::Finalized { .. } =>")
            && one_click_progress
                .contains("RuntimeTransactionRestoreExpectation::ExactPreservingCompensation")
            && one_click_progress.contains(
                "Self::Finalized { .. } => RuntimeTransactionRestoreExpectation::Exact(None)"
            )
            && one_click_writer
                .contains("let expected_record = progress.journaled_record().cloned()")
            && one_click_writer.contains("journal == expected")
            && one_click_writer.contains("RuntimeTransactionRecord::V2(next.clone())"),
        "one-click progress and checkpoint writer must retain complete-record ownership"
    );
    assert!(
        one_click_terminal_writers.contains("fn begin_one_click_finalize")
            && one_click_terminal_writers
                .contains("next.finalize = config::RuntimeFinalizeState::Intent")
            && one_click_terminal_writers.contains("fn complete_one_click_finalize")
            && one_click_terminal_writers.contains("fn replay_interrupted_one_click_finalize")
            && one_click_terminal_writers
                .contains("replay_finalize_authority_cleanup(state, ticket)")
            && one_click_terminal_writers
                .contains("current.runtime_binding = Some(binding.clone())")
            && one_click_terminal_writers.contains("current.runtime_transaction = None")
            && one_click_terminal_writers.contains("OneClickJournalProgress::Finalized")
            && source
                .contains("journal.compensation == config::RuntimeCompensationState::NotStarted")
            && source.contains(
                "journal.gateway_stop_outcome == config::RuntimeGatewayStopOutcome::NotAttempted"
            ),
        "one-click finalize writers must remain replayable complete-record transitions"
    );
    let compensation_contracts = [
        (
            "restore expectation",
            compensation_source.contains("journal_progress.restore_expectation()"),
        ),
        (
            "aggregate finish",
            compensation_journal.contains("fn finish_one_click_compensation"),
        ),
        (
            "step intent writer",
            compensation_journal.contains("fn begin_one_click_compensation_step"),
        ),
        (
            "step outcome writer",
            compensation_journal.contains("fn finish_one_click_compensation_step"),
        ),
        (
            "authority boundary writer",
            compensation_journal.contains("fn finish_one_click_authority_restore_step"),
        ),
        (
            "single expected business record",
            one_click_progress.contains("expected_runtime_transaction: Box<Option<")
                && compensation_journal
                    .contains("current.runtime_transaction != expected_runtime_transaction"),
        ),
        (
            "V2 schema",
            compensation_journal
                .contains("schema_version: config::RUNTIME_COMPENSATION_SCHEMA_VERSION_V2"),
        ),
        (
            "fixed pending plan",
            compensation_journal.contains("steps: config::pending_one_click_compensation_steps()"),
        ),
        (
            "aggregate in progress",
            compensation_journal.contains("state: config::RuntimeCompensationState::InProgress"),
        ),
        (
            "derived incomplete",
            compensation_journal
                .contains("record.state = config::RuntimeCompensationState::Incomplete"),
        ),
        (
            "successful clear",
            compensation_journal.contains("current.runtime_compensation = None"),
        ),
        (
            "initial durable publish",
            compensation_journal.contains("current.runtime_compensation = Some(next.clone())"),
        ),
        (
            "typed journal",
            compensation_journal.contains("RuntimeCompensationJournal"),
        ),
        (
            "business record owner",
            one_click_progress.contains("runtime_transaction: Box<Option<"),
        ),
        (
            "intent before effect planning",
            compensation_source.find("begin_one_click_compensation(")
                < compensation_source.find("let cross_runtime_environment"),
        ),
        (
            "production aggregate finish",
            compensation_source.contains("finish_one_click_compensation(dir, journal_progress)"),
        ),
    ];
    let missing_compensation_contracts = compensation_contracts
        .into_iter()
        .filter_map(|(name, present)| (!present).then_some(name))
        .collect::<Vec<_>>();
    assert!(
        missing_compensation_contracts.is_empty(),
        "durable compensation must publish and advance exact V2 step state around the production effect funnel; missing={missing_compensation_contracts:?}"
    );
    assert!(
        recovery_source
            .contains("RuntimeTransactionRestoreExpectation::ExactPreservingCompensation")
            && recovery_source.contains("current.runtime_compensation =")
            && recovery_source.contains("Some(compensation.clone())")
            && recovery_source
                .contains("current.runtime_transaction.as_ref() != expected.as_ref()")
            && restore_guard < first_restore_effect
            && first_restore_effect < confirmed_gateway_stop,
        "authority restore must preserve the exact durable compensation record before effects"
    );
    assert!(
        source.contains("fn config_authority_matches(")
            && source.contains("current.active_id == target_profile_id")
            && source.contains("current.runtime_binding.as_ref() == previous_binding")
            && one_click_writer.contains("config_authority_matches(")
            && one_click_terminal_writers
                .matches("config_authority_matches(")
                .count()
                >= 5
            && pending_cleanup_source.contains("cleanup_manifest_missing")
            && pending_cleanup_source.contains("manifest.schema_version == 2")
            && pending_cleanup_source.contains(
                "manifest.disposition == Some(PendingCleanupDisposition::CleanupOnly)"
            )
            && pending_cleanup_source.contains("cleanup_manifest_incomplete")
            && !pending_cleanup_source.contains(
                "else {\n        return Ok(FinalizeAuthorityReplayOutcome::Ready);"
            ),
        "handoff/finalize must CAS companion config authority and missing manifest must never count as durable cleanup completion"
    );
    let runtime_entry = source
        .split("pub(crate) fn one_click_login_entry")
        .nth(1)
        .and_then(|tail| tail.split("enum PriorScienceDisposition").next())
        .expect("production runtime entry facade must remain discoverable");
    assert!(
        command_projection_source.contains("one_click_login_entry(")
            && !command_projection_source.contains("recover_interrupted_gateway")
            && !command_projection_source.contains("replay_interrupted_one_click_finalize")
            && runtime_entry.contains("decide_one_click_entry_recovery(")
            && runtime_entry.contains("recover_interrupted_gateway")
            && runtime_entry.contains("replay_interrupted_one_click_finalize")
            && runtime_entry.contains("one_click_login_after_gateway_recovery"),
        "the runtime entry facade must exclusively own finalize/Gateway recovery and consuming handoff"
    );
    assert!(
        config_source.contains(
            "self.runtime_transaction.is_some() || self.runtime_compensation.is_some()"
        ) && config_source.contains("if cfg.has_open_runtime_journal()")
            && command_projection_source.contains("cfg.has_open_runtime_journal()")
            && profile_source.contains("if cfg.has_open_runtime_journal()")
            && finalize_consumer_source.contains("if cfg.has_open_runtime_journal()")
            && history_recovery_source.matches("has_open_runtime_journal()").count() >= 3
            && lifecycle_command_source
                .matches("config::require_no_runtime_transaction")
                .count()
                >= 4
            && codex_command_source
                .matches("config::require_no_runtime_transaction")
                .count()
                >= 5
            && profile_source
                .matches("config::require_no_runtime_transaction")
                .count()
                >= 7,
        "every normal entry, mutation guard, and read-model consumer must fail closed for either the business journal or the durable compensation journal"
    );
    assert!(
        gateway_recovery_source.contains(
            "#[derive(Debug, Eq, PartialEq)]\npub(crate) struct InterruptedGatewayTerminalHandoff"
        ) && gateway_recovery_source.contains("fn into_record(self)")
            && gateway_recovery_source.contains("fn into_terminal_record(self)")
            && !gateway_recovery_source.contains(
                "#[derive(Clone, Debug, Eq, PartialEq)]\npub(crate) struct InterruptedGatewayTerminalHandoff"
            )
            && !gateway_recovery_source.contains("pub(crate) fn record(&self)"),
        "the terminal Gateway handoff must remain non-Clone and expose only consuming transfer APIs"
    );
    let prior_intent = cold_source
        .find("let intent = begin_prior_stop_intent")
        .expect("prior Science durable intent must remain on the production path");
    let prior_stop = cold_source[prior_intent..]
        .find("stop_prior_science_with")
        .map(|index| index + prior_intent)
        .expect("prior Science exact stop must remain after durable intent");
    let prior_outcome = cold_source[prior_stop..]
        .find("publish_prior_stop_outcome")
        .map(|index| index + prior_stop)
        .expect("prior Science typed outcome must be published after the stop effect");
    assert!(
        prior_intent < prior_stop && prior_stop < prior_outcome,
        "durable PriorStopIntent must precede the exact stop and typed outcome publication"
    );
    assert!(
        transaction_stop_source.contains("struct TransactionScienceStopOwner")
            && transaction_stop_source
                .contains("owner.still_owns(&current, lifecycle.current_generation())")
            && transaction_stop_source.contains("fn execute_transaction_science_stop_with")
            && cold_source.contains("TransactionScienceStopBoundary::ColdPriorStop")
            && cold_source.contains("ScienceHostAdapter::execute_stop(&app, request)"),
        "cold prior Science stop must execute outside AppState and publish only through generation plus full-owner CAS"
    );
    let success_finalize = cold_source
        .rfind("begin_one_click_finalize(")
        .expect("success path must publish a finalize intent");
    let authority_conversion = cold_source[success_finalize..]
        .find("prepare_success(&mut value)")
        .map(|index| index + success_finalize)
        .expect("success finalize must convert authority to cleanup-only");
    let finalize_completion = cold_source[authority_conversion..]
        .find("complete_one_click_finalize")
        .map(|index| index + authority_conversion)
        .expect("success finalize must atomically commit binding and clear journal");
    assert!(
        success_finalize < authority_conversion && authority_conversion < finalize_completion,
        "success finalize must be intent -> authority conversion -> atomic binding/journal completion"
    );
    assert!(
        source.contains("fn preserve_interrupted_success_finalize(")
            && cold_source.matches("prepare_success(&mut value).is_err()").count() == 2
            && cold_source
                .matches("complete_one_click_finalize(&dir, &mut journal_progress).is_err()")
                .count()
                == 2
            && cold_source
                .matches("trace.finish(\"degraded=success_finalize_pending\");")
                .count()
                == 4
            && cold_source.matches("return Ok(value);").count() >= 4,
        "authority conversion or atomic completion failure must preserve the finalize journal and return degraded instead of entering legacy compensation"
    );
    let ordinary_constructor = source
        .split("fn typed_one_click_err")
        .nth(1)
        .and_then(|tail| tail.split("#[derive(Debug)]").next())
        .expect("ordinary one-click error constructor must remain discoverable");
    assert!(
        ordinary_constructor.contains("TypedOneClickFailure::new(kind, message)")
            && !ordinary_constructor.contains(".contains(")
            && !ordinary_constructor.contains("recovery_status")
            && !ordinary_constructor.contains("environment_uncertain"),
        "ordinary one-click errors must never infer recovery or environment from message text"
    );
    let command_projection = command_projection_source
        .split("pub(super) fn project_one_click_failure")
        .nth(1)
        .expect("one-click command projection must remain discoverable");
    assert!(
        command_projection.contains("apply_open_journal_degraded(journal_open)")
            && command_projection.contains("project_dto()")
            && !command_projection.contains("safe_detail")
            && !command_projection.contains(".contains(")
            && !command_projection.contains("recovery_from_diagnostic_codes"),
        "command projection must use typed fields and journal state only"
    );
    assert!(
        source.contains("typed_interrupted_science_err")
            && source.contains(".with_recovery(error.recovery)")
            && healthy_reopen_source.contains("primary.projected_recovery()"),
        "one-click and healthy reopen must preserve explicit typed recovery"
    );
    let profile_reconcile = profile_reconcile_source
        .split("if let Err(error) = crate::runtime::sandbox_session::reconcile_science_for_active")
        .nth(1)
        .and_then(|tail| {
            tail.split("} else {\n        let clear_result = config::update_result")
                .next()
        })
        .expect("profile reconcile projection must remain discoverable");
    assert!(
        profile_reconcile.contains("error.prior_science_restored()")
            && profile_reconcile.contains("error.environment_uncertain()")
            && !profile_reconcile.contains(".contains("),
        "profile reconcile must derive recovery and environment from typed variants"
    );
    let auto_boot_projection = auto_boot_source
        .split("fn run_boot_decision_with")
        .nth(1)
        .and_then(|tail| tail.split("fn run_boot_decision(").next())
        .expect("auto-boot projection must remain discoverable");
    assert!(
        auto_boot_projection.contains("project_consumer_state(&value)")
            && auto_boot_projection.contains("FinalizeConsumerDisposition::Ready")
            && auto_boot_projection.contains("FinalizeConsumerDisposition::Attention")
            && auto_boot_projection.contains("FinalizeConsumerDisposition::Manual")
            && auto_boot_projection.contains("mark_boot_attention(&app, value)")
            && auto_boot_projection.contains("mark_boot_failed(&app, value)")
            && !auto_boot_projection.contains("message")
            && !auto_boot_projection.contains(".contains("),
        "auto-boot must consume the typed readback projection while preserving the full DTO"
    );
    assert!(
        !source.contains("contains(\"recovery_status=cleanup_required\")")
            && !recovery_source.contains("contains(\"recovery_status=cleanup_required\")"),
        "authority cleanup recovery must be classified from typed results, not DTO text"
    );
    for field in [
        "science_cleanup: CompensationStepOutcome",
        "ssh_cleanup: CompensationStepOutcome",
        "authority_restore: CompensationStepOutcome",
        "prior_science_restart: CompensationStepOutcome",
        "snapshot_cleanup: CompensationStepOutcome",
        "environment: CompensationEnvironment",
    ] {
        assert!(
            compensation_source.contains(field),
            "CompensationOutcome must retain a typed result for {field}"
        );
    }
    let compensation_owner = compensation_source
        .split("fn compensate_one_click_failure")
        .nth(1)
        .expect("one-click compensation source boundary must remain discoverable");
    assert!(
        compensation_owner.contains("outcome.projected_recovery()")
            && compensation_owner.contains("outcome.render_failure_message"),
        "one-click compensation must derive recovery and diagnostics from CompensationOutcome"
    );
    assert!(
        !compensation_owner.contains("recovery_from_diagnostic_codes")
            && !compensation_owner.contains("message.contains"),
        "one-click compensation control flow must never parse rendered diagnostics"
    );
    let incomplete_after_prior_restart = CompensationOutcome {
        science_cleanup: CompensationStepOutcome::Skipped(
            CompensationSkipCause::NoScienceCandidate,
        ),
        ssh_cleanup: CompensationStepOutcome::Failed(CompensationCause::SshCleanup),
        authority_restore: CompensationStepOutcome::Succeeded,
        prior_science_restart: CompensationStepOutcome::Succeeded,
        snapshot_cleanup: CompensationStepOutcome::Skipped(
            CompensationSkipCause::SnapshotPreserved,
        ),
        environment: CompensationEnvironment::Quiescent,
    };
    assert!(
        !incomplete_after_prior_restart.authorities_restored()
            && !incomplete_after_prior_restart.prior_science_restored(),
        "a successful prior Science restart must not publish Restored when SSH cleanup failed"
    );
    assert_eq!(
        incomplete_after_prior_restart.projected_recovery(),
        ProjectedRecovery::DEGRADED,
        "incomplete pre-launch compensation must remain typed degraded"
    );
    let file = syn::parse_file(source).expect("one-click product Rust source must parse");
    let cold_file = syn::parse_file(cold_source).expect("cold one-click Rust source must parse");
    let compensation_file = syn::parse_file(compensation_source)
        .expect("one-click compensation phase Rust source must parse");
    let science_phase_file = syn::parse_file(science_phase_source)
        .expect("managed Science phase Rust source must parse");
    let cold = top_level(&cold_file, "run_cold_one_click")
        .expect("cold one-click coordinator must remain module-level");
    let science_phase = top_level(&science_phase_file, "run_managed_science_launch_phase")
        .expect("managed Science launch phase must remain module-level");
    let durable_recovery_restart = top_level(&file, "restart_science_identity_with_budget")
        .expect("fresh-process compensation must share the bounded Science restart owner");
    assert!(
        top_level(&compensation_file, "compensate_one_click_failure").is_some()
            && top_level(&file, "compensate_one_click_failure").is_none(),
        "cold compensation phase must exclusively own the release-visible failure helper"
    );
    let snapshot_index = cold
        .block
        .stmts
        .iter()
        .position(|statement| {
            matches!(
                statement,
                Stmt::Local(local)
                    if local_name(local).is_some_and(|name| name == "authority_transaction")
            )
        })
        .expect("one-click must capture AuthorityTransaction before mutation");
    let transaction_index = cold
        .block
        .stmts
        .iter()
        .position(|statement| {
            matches!(
                statement,
                Stmt::Local(local)
                    if local_name(local).is_some_and(|name| name == "transaction_result")
            )
        })
        .expect("one-click must contain transaction_result after snapshot identity freeze");
    assert_eq!(
        transaction_index,
        snapshot_index + 4,
        "AuthorityTransaction must be followed by the verified ticket, frozen V2 identity, PreJournalAbort progress, and transaction_result"
    );
    let frozen_identity_locals = cold.block.stmts[snapshot_index + 1..transaction_index]
        .iter()
        .map(|statement| match statement {
            Stmt::Local(local) => local_name(local).expect("identity freeze must use named locals"),
            _ => panic!("identity freeze boundary may contain only named local statements"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        frozen_identity_locals,
        ["snapshot_ticket", "transaction_identity", "journal_progress"],
        "snapshot ticket, transaction identity, and PreJournalAbort progress must be frozen exactly once before protected mutation"
    );
    assert_eq!(
        cold.block.stmts.len(),
        transaction_index + 2,
        "transaction_result must still be followed only by its final match"
    );
    let transaction_statement = &cold.block.stmts[transaction_index];
    let transaction_local = match transaction_statement {
        Stmt::Local(local)
            if local_name(local).is_some_and(|name| name == "transaction_result") =>
        {
            local
        }
        _ => {
            panic!("AuthorityTransaction identity freeze must be followed by transaction_result")
        }
    };
    let mut transaction_locals = TransactionLocalCount::default();
    transaction_locals.visit_item_fn(cold);
    assert_eq!(
        transaction_locals.0, 1,
        "one-click must contain exactly one transaction_result local"
    );
    let initializer = transaction_local
        .init
        .as_ref()
        .expect("transaction_result must have an immediate closure initializer");
    let transaction_call = match peel_expr(&initializer.expr) {
        Expr::Call(call) => call,
        _ => panic!("transaction_result initializer must directly invoke a closure"),
    };
    assert!(
        transaction_call.args.is_empty(),
        "transaction_result closure invocation must have zero arguments"
    );
    let transaction_closure = match peel_expr(&transaction_call.func) {
        Expr::Closure(closure) => closure,
        _ => panic!("transaction_result initializer must be a directly invoked closure"),
    };
    assert!(
        transaction_closure.inputs.is_empty(),
        "transaction_result closure must accept zero arguments"
    );
    let mut transaction = FlowFacts::default();
    transaction.visit_stmt(transaction_statement);
    assert_eq!(
        transaction.closures, 1,
        "transaction_result must be produced by one bounded mutation closure"
    );
    for required in [
        "ensure_virtual_login",
        "prepare_science_ssh_bridge",
        "revoke_science_ssh_bridge",
        "ensure_active",
        "run_managed_science_launch_phase",
    ] {
        assert!(
            transaction.calls.iter().any(|call| call == required),
            "the single mutation closure must own the {required} error edge"
        );
    }
    assert!(
        !transaction.calls.iter().any(|call| call == "spawn_launch")
            && !transaction
                .calls
                .iter()
                .any(|call| call == "accept_launch_script")
            && !transaction.calls.iter().any(|call| call == "verify_health")
            && !transaction
                .calls
                .iter()
                .any(|call| call == "verify_identity")
            && !transaction.calls.iter().any(|call| call == "commit_launch"),
        "the cold coordinator must delegate managed Science launch/health/receipt ownership"
    );
    assert!(
        transaction
            .methods
            .iter()
            .filter(|method| *method == "validate_science_restore_root")
            .count()
            >= 1,
        "cold orchestration must validate the exact Science restore root before protected writes"
    );
    let mut science_phase_flow = FlowFacts::default();
    science_phase_flow.visit_item_fn(science_phase);
    for required in [
        "spawn_launch",
        "accept_launch_script",
        "verify_health",
        "verify_identity",
        "commit_launch",
    ] {
        assert!(
            science_phase_flow.calls.iter().any(|call| call == required),
            "the managed Science phase must own the {required} edge"
        );
    }
    assert!(
        science_phase_flow
            .methods
            .iter()
            .filter(|method| *method == "validate_science_restore_root")
            .count()
            >= 2,
        "managed Science launch must revalidate exact Science/opaque-root bindings immediately before spawn"
    );
    assert!(
        !science_phase_flow.methods.iter().any(|method| method == "spawn")
            && !science_phase_flow.methods.iter().any(|method| method == "wait")
            && !science_phase_flow.methods.iter().any(|method| method == "status"),
        "the managed Science phase must use the typed ScienceHostAdapter instead of interpreting shell process methods"
    );
    let mut recovery_restart_flow = FlowFacts::default();
    recovery_restart_flow.visit_item_fn(durable_recovery_restart);
    for required in [
        "spawn_launch",
        "accept_launch_script",
        "verify_health",
        "verify_identity",
        "commit_launch",
    ] {
        assert!(
            recovery_restart_flow
                .calls
                .iter()
                .any(|call| call == required),
            "DB recovery restart must use the ScienceHostAdapter {required} phase"
        );
    }
    assert!(
        !recovery_restart_flow
            .methods
            .iter()
            .any(|method| method == "spawn")
            && !recovery_restart_flow
                .methods
                .iter()
                .any(|method| method == "try_wait")
            && !recovery_restart_flow
                .calls
                .iter()
                .any(|call| call == "http_health")
            && !recovery_restart_flow
                .methods
                .iter()
                .any(|method| method == "status"),
        "DB recovery coordinator must delegate shell wait and health timing to ScienceHostAdapter"
    );
    let host_adapter_source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/runtime/science/host_adapter.rs"),
    )
    .expect("ScienceHostAdapter source must be readable");
    let spawn_launch = host_adapter_source
        .split_once("pub(crate) fn spawn_launch")
        .map(|(_, suffix)| suffix)
        .expect("ScienceHostAdapter must define spawn_launch")
        .split("pub(crate) fn accept_launch_script")
        .next()
        .expect("spawn_launch must precede accept_launch_script");
    let deadline_offset = spawn_launch
        .find("let deadline =")
        .expect("recovery launch must freeze one absolute deadline");
    let command_offset = spawn_launch
        .find("Command::new")
        .expect("adapter must construct the Science launch command");
    let spawn_offset = spawn_launch
        .find(".spawn()")
        .expect("adapter must spawn the Science launch command");
    assert!(
        deadline_offset < command_offset && command_offset < spawn_offset,
        "the recovery absolute deadline must begin before command setup and cover spawn/wait"
    );
    let verify_health = host_adapter_source
        .split_once("pub(crate) fn verify_health")
        .map(|(_, suffix)| suffix)
        .expect("ScienceHostAdapter must define verify_health")
        .split("pub(crate) fn verify_identity")
        .next()
        .expect("verify_health must precede verify_identity");
    assert!(
        verify_health.contains("Some(deadline)")
            && verify_health.contains("saturating_duration_since")
            && verify_health.contains("remaining.as_millis()"),
        "the same recovery deadline must cap health polling and each remaining probe timeout"
    );
    assert!(
        !transaction.methods.iter().any(|method| method == "commit")
            && !transaction
                .calls
                .iter()
                .any(|call| call == "compensate_one_click_failure"),
        "transaction_result closure must neither commit nor compensate its own snapshot"
    );

    let final_statement = cold
        .block
        .stmts
        .last()
        .expect("one-click must end in the transaction result match");
    let final_match = match final_statement {
        Stmt::Expr(Expr::Match(expression), _) => expression,
        _ => panic!("one-click must end with exactly one success/failure transaction match"),
    };
    assert!(
        matches!(
            &*final_match.expr,
            Expr::Path(path) if path.path.is_ident("transaction_result")
        ),
        "the final transaction match must consume transaction_result directly"
    );
    assert_eq!(
        final_match.arms.len(),
        2,
        "the final transaction match must contain only one success and one failure arm"
    );
    assert!(
        final_match.arms.iter().all(|arm| arm.guard.is_none()),
        "the final transaction match must not use guarded arms"
    );
    let success = final_match
        .arms
        .iter()
        .find(|arm| result_arm_name(&arm.pat).is_some_and(|name| name == "Ok"))
        .expect("the final transaction match must contain one Ok arm");
    let failure = final_match
        .arms
        .iter()
        .find(|arm| result_arm_name(&arm.pat).is_some_and(|name| name == "Err"))
        .expect("the final transaction match must contain one Err arm");
    let success_block = match peel_expr(&success.body) {
        Expr::Block(block) => &block.block,
        _ => panic!("Ok arm must be a block containing commit and an infallible tail"),
    };
    assert_eq!(
        success_block.stmts.len(),
        2,
        "Ok arm must contain only snapshot commit and an infallible success tail"
    );
    let direct_commit = match &success_block.stmts[0] {
        Stmt::Expr(Expr::MethodCall(call), Some(_)) => {
            call.method == "commit"
                && call.args.is_empty()
                && matches!(
                    peel_expr(&call.receiver),
                    Expr::Path(path) if path.path.is_ident("authority_transaction")
                )
        }
        _ => false,
    };
    assert!(
        direct_commit,
        "Ok arm must begin with the sole direct authority_transaction.commit()"
    );
    assert!(
        matches!(
            &success_block.stmts[1],
            Stmt::Expr(tail, None) if success_tail_is_infallible(tail)
        ),
        "Ok arm must end with only an infallible path or Ok(path) tail"
    );

    let failure_expression = match peel_expr(&failure.body) {
        Expr::Block(block) if matches!(block.block.stmts.as_slice(), [Stmt::Expr(_, None)]) => {
            match &block.block.stmts[0] {
                Stmt::Expr(expression, None) => expression,
                _ => unreachable!(),
            }
        }
        expression => expression,
    };
    assert!(
        direct_call_name(failure_expression)
            .is_some_and(|name| name == "compensate_one_click_failure"),
        "Err arm must be exactly one direct compensate_one_click_failure call"
    );
    let Expr::Call(failure_call) = peel_expr(failure_expression) else {
        unreachable!()
    };
    let mut failure_arguments = OuterFacts::default();
    for argument in &failure_call.args {
        failure_arguments.visit_expr(argument);
    }
    assert!(
        failure_arguments.calls.is_empty()
            && failure_arguments.methods.is_empty()
            && failure_arguments.tries == 0
            && failure_arguments.returns == 0
            && failure_arguments.assignments == 0
            && failure_arguments.macros == 0
            && failure_arguments.closures == 0,
        "Err compensation arguments must be operation-free: {failure_arguments:?}"
    );

    let mut post_snapshot = FlowFacts::default();
    for statement in cold.block.stmts.iter().skip(snapshot_index + 1) {
        post_snapshot.visit_stmt(statement);
    }
    assert_eq!(
        post_snapshot
            .methods
            .iter()
            .filter(|method| *method == "commit")
            .count(),
        1,
        "all post-snapshot AST must contain exactly one commit, solely in Ok"
    );
    assert_eq!(
        post_snapshot
            .calls
            .iter()
            .filter(|call| *call == "compensate_one_click_failure")
            .count(),
        1,
        "all post-snapshot AST must contain exactly one compensation call, solely in Err"
    );
}

#[test]
fn transaction_scoped_science_stop_owner_covers_every_durable_boundary() {
    let owner_source = include_str!("../transaction_science_stop.rs");
    let one_click_source = include_str!("../one_click.rs");
    let cold_source = include_str!("../one_click/cold.rs");
    let science_phase_source = include_str!("../one_click/cold/science_phase.rs");
    let compensation_source = include_str!("../one_click/cold/compensation.rs");
    let replay_source = include_str!("../one_click/compensation_replay.rs");
    let history_source = include_str!("../history_recovery.rs");
    let runtime_journal_source = include_str!("runtime_journal.rs");

    let owner = owner_source
        .split("pub(super) fn execute_transaction_science_stop_with")
        .nth(1)
        .expect("transaction-scoped Science stop owner must remain discoverable");
    for field in [
        "generation: u64",
        "runtime: Option<ScienceRuntimeIdentity>",
        "confirmed_stopped: Option<ScienceRuntimeIdentity>",
        "sandbox_child_pid: Option<u32>",
        "sandbox_port: u16",
        "sandbox_url: Option<String>",
    ] {
        assert!(owner_source.contains(field), "stop owner lost {field}");
    }
    let freeze = owner
        .find("let owner =")
        .expect("owner freeze must remain explicit");
    let probe = owner
        .find("claim_exact_request()")
        .expect("exact request probe must remain explicit");
    let pre_execute_cas = owner[probe..]
        .find("owner.still_owns(&current, lifecycle.current_generation())")
        .map(|index| index + probe)
        .expect("owner must be rechecked after the external probe");
    let execute = owner
        .find("let execution = request.map")
        .expect("stop execution must remain explicit");
    let publish = owner
        .find("let mut current = lock(state)")
        .expect("CAS publication must reacquire AppState");
    assert!(
        freeze < probe && probe < pre_execute_cas && pre_execute_cas < execute && execute < publish,
        "probe and exact stop must remain outside AppState with a pre-execute owner recheck"
    );
    assert!(
        owner.contains("};\n    crate::runtime::system::kill_child(&mut stopped_child);"),
        "tracked-child KILL/wait must run only after the CAS publication lock is released"
    );
    assert!(
        owner.contains("owner.still_owns(&current, lifecycle.current_generation())")
            && owner.contains("verified.require_exact_stop_of(expected_runtime)")
            && owner.contains("已保留 replacement runtime"),
        "publication must require generation plus full owner identity and exact stop proof"
    );
    assert!(!owner_source.contains("fn stop_sandbox_state"));
    assert!(!cold_source.contains("ColdPriorScienceStopOwner"));
    assert!(!owner_source.contains("fn restart_managed_science_with_budget"));

    let profile_rollback = one_click_source
        .split("pub(crate) fn force_restart_science_for_active")
        .nth(1)
        .and_then(|tail| tail.split("fn typed_one_click_err").next())
        .expect("profile-switch rollback stop boundary must remain discoverable");
    let history_prior_stop = history_source
        .split("pub(crate) fn restore_history_choice_entry")
        .nth(1)
        .expect("history prior-stop boundary must remain discoverable");
    let live_compensation = compensation_source
        .split("fn compensate_one_click_failure")
        .nth(1)
        .expect("live compensation stop boundary must remain discoverable");
    let fresh_replay = replay_source
        .split("fn replay_science_cleanup")
        .nth(1)
        .and_then(|tail| tail.split("fn replay_ssh_cleanup").next())
        .expect("fresh compensation replay stop boundary must remain discoverable");
    for (name, source, boundary) in [
        (
            "managed DB restart",
            science_phase_source,
            "ManagedDbRestart",
        ),
        (
            "profile-switch rollback",
            profile_rollback,
            "ProfileSwitchRollback",
        ),
        (
            "history recovery prior stop",
            history_prior_stop,
            "HistoryRecoveryPriorStop",
        ),
        (
            "live compensation cleanup",
            live_compensation,
            "LiveCompensationCleanup",
        ),
        (
            "fresh compensation replay cleanup",
            fresh_replay,
            "CompensationReplayCleanup",
        ),
    ] {
        assert!(
            source.contains("execute_transaction_science_stop_with")
                && source.contains(&format!("TransactionScienceStopBoundary::{boundary}"))
                && source.contains("ScienceStopRequest::exact"),
            "{name} must freeze an exact request and use the shared owner/wait/CAS boundary"
        );
    }

    let history_intent = history_prior_stop
        .find("begin_history_transaction")
        .expect("history stop intent must be durable");
    let history_effect = history_prior_stop
        .find("execute_transaction_science_stop_with")
        .expect("history exact stop effect must exist");
    let history_outcome = history_prior_stop
        .find("publish_stop_outcome")
        .expect("history stop outcome must be durable");
    assert!(history_intent < history_effect && history_effect < history_outcome);

    let live_intent = live_compensation
        .find("persist_compensation_step_intent")
        .expect("live compensation step intent must be durable");
    let live_effect = live_compensation
        .find("TransactionScienceStopBoundary::LiveCompensationCleanup")
        .expect("live compensation cleanup effect must exist");
    let live_outcome = live_compensation
        .find("persist_compensation_step_outcome")
        .expect("live compensation step outcome must be durable");
    assert!(live_intent < live_effect && live_effect < live_outcome);

    let replay_owner = replay_source
        .split("fn replay_interrupted_one_click_compensation")
        .nth(1)
        .and_then(|tail| tail.split("fn validate_replay_manifest").next())
        .expect("fresh replay owner must remain discoverable");
    assert!(
        replay_owner
            .find("begin_one_click_compensation_step")
            .unwrap()
            < replay_owner.find("replay_science_cleanup").unwrap()
            && replay_owner.find("replay_science_cleanup").unwrap()
                < replay_owner
                    .find("finish_one_click_compensation_step")
                    .unwrap(),
        "fresh replay must retain durable step intent -> effect -> outcome order"
    );
    assert!(
        runtime_journal_source
            .contains("fixture must crash after authority effect but before durable outcome")
            && runtime_journal_source
                .contains("o1_e3_fresh_process_replays_durable_compensation_to_convergence")
            && runtime_journal_source
                .contains("fresh replay after convergence must be an idempotent no-op"),
        "compensation/replay crash and idempotence fixtures must remain live"
    );
    assert!(
        runtime_journal_source.contains(
            "fixture boundary must retain the pre-effect durable intent after the restart effect"
        ) && runtime_journal_source
            .contains("fresh replay must hydrate the V2 receipt before publishing AppState"),
        "legacy prior restart replay must retain its crash boundary and V2 hydration fixture"
    );
}

#[test]
fn o1_e3_compensation_replay_has_one_durable_pre_auth_owner() {
    let command_source = include_str!("../../../commands/runtime/one_click.rs");
    let compensation_source = include_str!("../one_click/cold/compensation.rs");
    let replay_source = include_str!("../one_click/compensation_replay.rs");
    let owner_source = include_str!("../one_click.rs");
    let recovery_source = include_str!("../recovery.rs");
    let managed_launch_source = include_str!("../../science/managed_launch.rs");
    let settings_source = include_str!("../../settings.rs");
    let config_source = include_str!("../../../config.rs");

    let persist_private = compensation_source
        .find("persist_compensation_replay_manifest(")
        .expect("compensation must persist its private replay manifest");
    let publish_intent = compensation_source
        .find("begin_one_click_compensation_with_id(")
        .expect("compensation must publish its public exact-step intent");
    assert!(
        persist_private < publish_intent,
        "private replay authority must be durable before public compensation intent"
    );
    assert!(
        compensation_source
            .find("acquire_runtime_compensation_replay_lease")
            .unwrap()
            < compensation_source
                .find("\n    persist_compensation_step_intent")
                .unwrap(),
        "live compensation must join the cross-process effect lease before its first step intent"
    );

    let replay_before_auth = command_source
        .find("match replay_compensation_before_auth")
        .expect("command entry must own interrupted compensation replay");
    let provider_auth = command_source
        .find("prepare_provider_auth(")
        .expect("provider auth preflight must remain discoverable");
    assert!(
        replay_before_auth < provider_auth,
        "durable compensation must converge before provider auth"
    );
    assert!(
        command_source.contains("interrupted_compensation_requires_pre_auth_replay()")
            && command_source.contains("RuntimeMutationDomain::Destructive")
            && command_source.contains("acquire_runtime_compensation_auth_lease")
            && owner_source.contains("acquire_runtime_compensation_publication_lease"),
        "normal login must avoid the destructive lease while replay must serialize effects"
    );

    assert!(
        replay_source.contains("read_registered_private_manifest")
            && replay_source.contains("OneClickAuthoritySnapshot::load_durable")
            && replay_source.contains("decide_compensation_replay")
            && replay_source
                .find("acquire_runtime_compensation_replay_lease")
                .unwrap()
                < replay_source.find("let cfg = config::load_from").unwrap()
            && recovery_source.contains("restore_durable_authority"),
        "fresh replay must reconstruct exact authority and typed step progress from durable state"
    );
    assert!(
        replay_source.contains("ssh_stub_transaction")
            && replay_source.contains("transaction.compensate_durable")
            && settings_source.contains("fn compensate_durable")
            && replay_source.contains("prior_restart_launch_id")
            && replay_source.contains("hydrate_runtime_from_v2_managed_launch")
            && managed_launch_source.contains("token.record.schema_version != 2")
            && managed_launch_source.contains("managed_launch_token_is_current_for_runtime")
            && replay_source.contains("prior_restart_receipt_is_absent")
            && recovery_source.contains("let already_restored = current == restored_config")
            && config_source.contains("RUNTIME_COMPENSATION_AUTH_LOCK_FILE"),
        "SSH, authority, prior Science and auth races must retain exact durable identities"
    );
    assert!(
        !replay_source.contains("message.contains") && !replay_source.contains("restore_app_state"),
        "diagnostics and stale process-local AppState must not control fresh replay"
    );
}

#[test]
fn o1_e4_history_full_snapshot_restore_has_one_durable_effect_owner() {
    let history_source = include_str!("../history_recovery.rs");
    let config_source = include_str!("../../../config.rs");
    let live = history_source
        .split("pub(crate) fn restore_history_choice_entry")
        .nth(1)
        .expect("history restore entry must remain discoverable");
    let replay = history_source
        .split("pub(crate) fn replay_interrupted_history_recovery(")
        .nth(1)
        .expect("history replay entry must remain discoverable");
    let begin = history_source
        .find("fn begin_history_authority_restore(")
        .expect("history replay must persist an authority-restore intent");
    let effect = history_source
        .find("restore_history_authority_from_manifest(&pending, state)?")
        .expect("history replay must execute the exact full-snapshot restore");
    let finish = history_source
        .find("finish_history_authority_restore(&config::default_dir(), &pending)")
        .expect("history replay must persist the authority-restore outcome");

    assert!(
        live.find("acquire_runtime_history_effect_lease").unwrap()
            < live.find("let owned_cfg = config::load_from").unwrap()
            && live.find("let owned_cfg = config::load_from").unwrap()
                < live.find("oauth_forge::restore_history_choice(").unwrap(),
        "live history publication must acquire and revalidate its cross-process effect owner before credential effects"
    );
    assert!(
        replay.find("acquire_runtime_history_effect_lease").unwrap()
            < replay.find("let cfg = config::load_from").unwrap(),
        "fresh history replay must acquire the same effect owner before recapturing durable state"
    );
    assert!(
        begin < effect && effect < finish,
        "history full-snapshot restore must persist intent before effect and outcome after effect"
    );
    assert!(
        config_source.contains("HistoryAuthorityRestorePending")
            && config_source.contains("HistoryAuthorityRestoreSucceeded")
            && config_source.contains("acquire_runtime_history_effect_lease")
            && config_source.contains("RUNTIME_COMPENSATION_AUTH_LOCK_FILE"),
        "history restore phases and the shared crash-releasing fence must remain durable"
    );
    assert!(
        !history_source.contains("message.contains"),
        "history restore diagnostics must not control replay"
    );
}
