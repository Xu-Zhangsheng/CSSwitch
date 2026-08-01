#[test]
fn one_click_snapshot_has_one_commit_and_one_failure_compensation_funnel() {
    use syn::visit::{self, Visit};
    use syn::{Expr, ExprCall, ExprMethodCall, Item, ItemFn, Pat, Stmt};

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
    let recovery_source = include_str!("../recovery.rs");
    assert!(
        !source.contains("contains(\"recovery_status=cleanup_required\")")
            && !recovery_source.contains("contains(\"recovery_status=cleanup_required\")"),
        "authority cleanup recovery must be classified from typed results, not DTO text"
    );
    let file = syn::parse_file(source).expect("one-click product Rust source must parse");
    let one_click = top_level(&file, "one_click_login_with_options")
        .expect("one-click product function must remain module-level");
    let recovery_restart = top_level(&file, "restart_managed_science_with_budget")
        .expect("DB recovery restart must remain a module-level bounded helper");
    assert!(
        top_level(&file, "compensate_one_click_failure").is_some(),
        "one-click must expose one release-visible failure compensation helper"
    );
    let snapshot_index = one_click
        .block
        .stmts
        .iter()
        .position(|statement| {
            matches!(
                statement,
                Stmt::Local(local)
                    if local_name(local).is_some_and(|name| name == "authority_snapshot")
            )
        })
        .expect("one-click must capture authority_snapshot before mutation");
    assert_eq!(
        one_click.block.stmts.len(),
        snapshot_index + 3,
        "authority_snapshot must be followed by exactly transaction_result and its final match"
    );
    let transaction_index = snapshot_index + 1;
    let transaction_statement = &one_click.block.stmts[transaction_index];
    let transaction_local = match transaction_statement {
        Stmt::Local(local)
            if local_name(local).is_some_and(|name| name == "transaction_result") =>
        {
            local
        }
        _ => {
            panic!("authority_snapshot must be followed immediately by let transaction_result")
        }
    };
    let mut transaction_locals = TransactionLocalCount::default();
    transaction_locals.visit_item_fn(one_click);
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
        "ensure_proxy",
        "record_managed_science_launch",
    ] {
        assert!(
            transaction.calls.iter().any(|call| call == required),
            "the single mutation closure must own the {required} error edge"
        );
    }
    assert!(
            transaction.methods.iter().any(|method| method == "spawn")
                && transaction.methods.iter().any(|method| method == "wait")
                && !transaction.methods.iter().any(|method| method == "status"),
            "the single mutation closure must own explicit shell spawn/wait and distinguish spawn from wait failure"
        );
    assert!(
            transaction
                .methods
                .iter()
                .filter(|method| *method == "validate_science_restore_root")
                .count()
                >= 3,
            "one-click must revalidate exact Science/opaque-root bindings before protected writes and immediately before spawn"
        );
    let mut recovery_restart_flow = FlowFacts::default();
    recovery_restart_flow.visit_item_fn(recovery_restart);
    assert!(
            recovery_restart_flow
                .methods
                .iter()
                .any(|method| method == "spawn")
                && recovery_restart_flow
                    .methods
                    .iter()
                    .any(|method| method == "try_wait")
                && recovery_restart_flow
                    .methods
                    .iter()
                    .any(|method| method == "saturating_duration_since")
                && recovery_restart_flow
                    .calls
                    .iter()
                    .any(|call| call == "http_health")
                && !recovery_restart_flow
                    .methods
                    .iter()
                    .any(|method| method == "status"),
            "DB recovery restart must enforce one absolute deadline across explicit shell try_wait and remaining-time-capped health"
        );
    assert!(
        !transaction.methods.iter().any(|method| method == "commit")
            && !transaction
                .calls
                .iter()
                .any(|call| call == "compensate_one_click_failure"),
        "transaction_result closure must neither commit nor compensate its own snapshot"
    );

    let final_statement = one_click
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
                    Expr::Path(path) if path.path.is_ident("authority_snapshot")
                )
        }
        _ => false,
    };
    assert!(
        direct_commit,
        "Ok arm must begin with the sole direct authority_snapshot.commit()"
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
    for statement in one_click.block.stmts.iter().skip(snapshot_index + 1) {
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
