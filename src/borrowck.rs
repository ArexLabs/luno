use std::collections::{HashMap, HashSet};
use crate::ast::*;
use crate::checker::Checker;
use crate::error::{Diag, Diagnostics, Span};
use crate::types::Type;

pub struct BorrowChecker<'a> {
    checker: &'a Checker,
    diags: Diagnostics,

    moved: HashSet<String>,
    imm_borrows: HashMap<String, Vec<(String, Span)>>,
    mut_borrow: HashMap<String, (String, Span)>,
    ref_targets: HashMap<String, (String, bool)>,
    scope_stack: Vec<Vec<String>>,
    var_types: HashMap<String, Type>,

    fn_sigs: HashMap<String, Vec<(String, Type)>>,
    method_sigs: HashMap<(String, String), Vec<(String, Type)>>,

    fn_ret_type: Option<Type>,
}

impl<'a> BorrowChecker<'a> {
    pub fn new(checker: &'a Checker, program: &Program) -> Self {
        let mut sigs: HashMap<String, Vec<(String, Type)>> = HashMap::new();
        let mut msigs: HashMap<(String, String), Vec<(String, Type)>> = HashMap::new();

        for stmt in &program.stmts {
            match stmt {
                Stmt::FnDef { name, params, .. } => {
                    let ptypes: Vec<(String, Type)> = params.iter()
                        .filter_map(|p| {
                            p.type_hint.as_ref().map(|t| {
                                (p.name.clone(), Self::resolve_texpr_to_type(t, &checker.types))
                            })
                        })
                        .collect();
                    sigs.insert(name.clone(), ptypes);
                }
                Stmt::ImplBlock { type_name, methods, .. } => {
                    for m in methods {
                        if let Stmt::FnDef { name, params, .. } = m {
                            let ptypes: Vec<(String, Type)> = params.iter()
                                .filter_map(|p| {
                                    p.type_hint.as_ref().map(|t| {
                                        (p.name.clone(), Self::resolve_texpr_to_type(t, &checker.types))
                                    })
                                })
                                .collect();
                            msigs.insert((type_name.clone(), name.clone()), ptypes);
                        }
                    }
                }
                _ => {}
            }
        }

        BorrowChecker {
            checker,
            diags: Diagnostics::new(),
            moved: HashSet::new(),
            imm_borrows: HashMap::new(),
            mut_borrow: HashMap::new(),
            ref_targets: HashMap::new(),
            scope_stack: vec![vec![]],
            var_types: HashMap::new(),
            fn_sigs: sigs,
            method_sigs: msigs,
            fn_ret_type: None,
        }
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), Diagnostics> {
        for stmt in &program.stmts {
            if let Err(()) = self.check_stmt(stmt) {
                break;
            }
        }
        if self.diags.has_errors() {
            Err(std::mem::take(&mut self.diags))
        } else {
            Ok(())
        }
    }

    pub fn has_errors(&self) -> bool {
        self.diags.has_errors()
    }

    pub fn diags_mut(&mut self) -> &mut Diagnostics {
        &mut self.diags
    }

    // ---------- helpers ----------

    fn resolve_texpr_to_type(texpr: &TypeExpr, types: &crate::types::TypeTable) -> Type {
        match texpr {
            TypeExpr::Named(name, _) => {
                types.types.get(name).cloned().unwrap_or(Type::Named(name.clone()))
            }
            TypeExpr::Generic(name, args, _) => {
                let resolved: Vec<Type> = args.iter()
                    .map(|a| Self::resolve_texpr_to_type(a, types))
                    .collect();
                Type::Generic(name.clone(), resolved)
            }
            TypeExpr::Ref(inner, _) => Type::Ref(Box::new(Self::resolve_texpr_to_type(inner, types))),
            TypeExpr::MutRef(inner, _) => Type::MutRef(Box::new(Self::resolve_texpr_to_type(inner, types))),
            TypeExpr::Infer(_) => Type::Int,
            TypeExpr::FnType(params, ret, _) => {
                let p: Vec<Type> = params.iter().map(|p| Self::resolve_texpr_to_type(p, types)).collect();
                Type::Fn(p, ret.as_ref().map(|r| Box::new(Self::resolve_texpr_to_type(r, types))))
            }
            TypeExpr::Tuple(ts, _) => {
                Type::Tuple(ts.iter().map(|t| Self::resolve_texpr_to_type(t, types)).collect())
            }
        }
    }

    fn is_copy_type(ty: &Type) -> bool {
        matches!(ty, Type::Int | Type::Float | Type::Bool | Type::Char | Type::Byte)
    }

    fn is_ref_type(ty: &Type) -> bool {
        matches!(ty, Type::Ref(_) | Type::MutRef(_))
    }

    fn expr_to_type(&self, expr: &Expr) -> Type {
        match expr {
            Expr::Literal(lit, _) => match lit {
                Literal::Int(_) => Type::Int,
                Literal::Float(_) => Type::Float,
                Literal::Bool(_) => Type::Bool,
                Literal::Char(_) => Type::Char,
                Literal::String(_) => Type::String,
                Literal::Null => Type::Never,
            },
            Expr::Ident(name, _) => {
                self.var_types.get(name).cloned()
                    .or_else(|| self.checker.vars.get(name).cloned())
                    .unwrap_or(Type::Named(name.clone()))
            }
            Expr::Call(callee, _, _) => {
                if let Expr::Ident(n, _) = callee.as_ref() {
                    if n == "make" { return Type::Chan(Box::new(Type::Never)); }
                }
                Type::Int
            }
            Expr::Spawn(_, _) => Type::Future(Box::new(Type::Never)),
            Expr::Make(_, _, _) => Type::Chan(Box::new(Type::Never)),
            Expr::StructLit(..) => Type::String,
            Expr::EnumVariant(..) => Type::String,
            _ => Type::Int,
        }
    }

    fn enter_scope(&mut self) {
        self.scope_stack.push(vec![]);
    }

    fn exit_scope(&mut self) {
        if let Some(scope) = self.scope_stack.pop() {
            for binding in scope {
                if let Some((target, is_mut)) = self.ref_targets.remove(&binding) {
                    if is_mut {
                        self.mut_borrow.remove(&target);
                    } else if let Some(borrows) = self.imm_borrows.get_mut(&target) {
                        borrows.retain(|(h, _)| h != &binding);
                        if borrows.is_empty() {
                            self.imm_borrows.remove(&target);
                        }
                    }
                }
            }
        }
    }

    fn track_binding(&mut self, binding: &str) {
        if let Some(scope) = self.scope_stack.last_mut() {
            scope.push(binding.to_string());
        }
    }

    fn mark_moved(&mut self, var: &str, span: &Span) {
        if self.moved.contains(var) { return; }
        if self.imm_borrows.contains_key(var) {
            self.diags.push(Diag::error(
                format!("cannot move out of `{}` while immutably borrowed", var),
                span.clone(),
            ));
            return;
        }
        if self.mut_borrow.contains_key(var) {
            self.diags.push(Diag::error(
                format!("cannot move out of `{}` while mutably borrowed", var),
                span.clone(),
            ));
            return;
        }
        self.moved.insert(var.to_string());
    }

    fn check_var_read(&mut self, var: &str, span: &Span) {
        if self.moved.contains(var) {
            return;
        }
        if self.mut_borrow.contains_key(var) {
            self.diags.push(Diag::error(
                format!("cannot use `{}` while mutably borrowed", var),
                span.clone(),
            ));
        }
    }

    fn check_var_write(&mut self, var: &str, span: &Span) {
        if self.mut_borrow.contains_key(var) {
            self.diags.push(Diag::error(
                format!("cannot assign to `{}` because it is mutably borrowed", var),
                span.clone(),
            ));
            return;
        }
        if self.imm_borrows.contains_key(var) {
            self.diags.push(Diag::error(
                format!("cannot assign to `{}` because it is immutably borrowed", var),
                span.clone(),
            ));
            return;
        }
        self.moved.remove(var);
    }

    fn add_imm_borrow(&mut self, target: &str, holder: &str, span: &Span) {
        if self.moved.contains(target) {
            self.diags.push(Diag::error(
                format!("cannot borrow `{}` after move", target),
                span.clone(),
            ));
            return;
        }
        if self.mut_borrow.contains_key(target) {
            self.diags.push(Diag::error(
                format!("cannot borrow `{}` as immutable because it is also borrowed as mutable", target),
                span.clone(),
            ));
            return;
        }
        self.imm_borrows
            .entry(target.to_string())
            .or_default()
            .push((holder.to_string(), span.clone()));
    }

    fn add_mut_borrow(&mut self, target: &str, holder: &str, span: &Span) {
        if self.moved.contains(target) {
            self.diags.push(Diag::error(
                format!("cannot borrow `{}` after move", target),
                span.clone(),
            ));
            return;
        }
        if self.mut_borrow.contains_key(target) {
            self.diags.push(Diag::error(
                format!("cannot borrow `{}` as mutable more than once", target),
                span.clone(),
            ));
            return;
        }
        if self.imm_borrows.contains_key(target) {
            self.diags.push(Diag::error(
                format!("cannot borrow `{}` as mutable because it is also borrowed as immutable", target),
                span.clone(),
            ));
            return;
        }
        self.mut_borrow
            .insert(target.to_string(), (holder.to_string(), span.clone()));
    }

    // ---------- statement checking ----------

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<(), ()> {
        match stmt {
            Stmt::Let { name, value, type_hint, .. } => {
                if let Some(t) = type_hint {
                    let ty = Self::resolve_texpr_to_type(t, &self.checker.types);
                    self.var_types.insert(name.clone(), ty);
                } else {
                    let ty = self.expr_to_type(value);
                    self.var_types.insert(name.clone(), ty);
                }

                self.check_expr(value, false)?;

                if let Expr::Ident(ident, ispan) = value {
                    if let Some(ty) = self.var_types.get(ident) {
                        if !Self::is_copy_type(ty) && !Self::is_ref_type(ty) {
                            self.mark_moved(ident, ispan);
                        }
                    }
                }
                Ok(())
            }
            Stmt::Const { name, value, .. } => {
                let ty = self.expr_to_type(value);
                self.var_types.insert(name.clone(), ty);
                self.check_expr(value, false)?;
                Ok(())
            }
            Stmt::Assign { target, value, .. } => {
                self.check_expr(value, false)?;
                self.check_expr(target, true)?;
                Ok(())
            }
            Stmt::Expr(expr) => {
                self.check_expr(expr, false)?;
                Ok(())
            }
            Stmt::Return(expr, _) => {
                if let Some(e) = expr {
                    self.check_expr(e, false)?;
                }
                Ok(())
            }
            Stmt::Break(_) | Stmt::Continue(_) => Ok(()),

            Stmt::FnDef { name: _, params, return_type, body, .. } => {
                let old_moved = self.moved.clone();
                let old_imm = self.imm_borrows.clone();
                let old_mut = self.mut_borrow.clone();
                let old_refs = self.ref_targets.clone();
                let old_vars = self.var_types.clone();
                let old_ret = self.fn_ret_type.clone();
                let old_scopes = self.scope_stack.clone();

                self.moved.clear();
                self.imm_borrows.clear();
                self.mut_borrow.clear();
                self.ref_targets.clear();
                self.scope_stack = vec![vec![]];

                for param in params {
                    if let Some(ref t) = param.type_hint {
                        let ty = Self::resolve_texpr_to_type(t, &self.checker.types);
                        self.var_types.insert(param.name.clone(), ty);
                    }
                }

                self.fn_ret_type = return_type.as_ref()
                    .map(|t| Self::resolve_texpr_to_type(t, &self.checker.types));

                for s in body {
                    if self.diags.has_errors() { break; }
                    self.check_stmt(s)?;
                }

                self.moved = old_moved;
                self.imm_borrows = old_imm;
                self.mut_borrow = old_mut;
                self.ref_targets = old_refs;
                self.var_types = old_vars;
                self.fn_ret_type = old_ret;
                self.scope_stack = old_scopes;

                Ok(())
            }

            Stmt::TypeDef { .. } | Stmt::EnumDef { .. } | Stmt::ImplBlock { .. }
            | Stmt::ImplTrait { .. } | Stmt::TraitDef { .. }
            | Stmt::Import { .. } | Stmt::FromImport { .. } => Ok(()),
        }
    }

    // ---------- expression checking ----------
    // write_ctx: true if expression appears in a write position (LHS of assignment)

    fn check_expr(&mut self, expr: &Expr, write_ctx: bool) -> Result<(), ()> {
        match expr {
            Expr::Literal(_, _) => Ok(()),

            Expr::Ident(name, span) => {
                if write_ctx {
                    self.check_var_write(name, span);
                } else {
                    self.check_var_read(name, span);
                    if self.diags.has_errors() { return Err(()); }
                    if self.moved.contains(name.as_str()) {
                        self.diags.push(Diag::error(
                            format!("use of moved value: `{}`", name),
                            span.clone(),
                        ));
                    }
                }
                Ok(())
            }

            Expr::BinOp(left, _, right, _) => {
                self.check_expr(left, false)?;
                self.check_expr(right, false)?;
                Ok(())
            }

            Expr::UnaryOp(op, operand, span) => {
                match op {
                    UnaryOp::Ref => {
                        if let Expr::Ident(var, _) = operand.as_ref() {
                            let holder = format!("_ref_{}", self.scope_stack.len());
                            self.track_binding(&holder);
                            self.add_imm_borrow(var, &holder, span);
                        } else {
                            self.check_expr(operand, false)?;
                        }
                        Ok(())
                    }
                    UnaryOp::MutRef => {
                        if let Expr::Ident(var, _) = operand.as_ref() {
                            let holder = format!("_mut_{}", self.scope_stack.len());
                            self.track_binding(&holder);
                            self.add_mut_borrow(var, &holder, span);
                        } else {
                            self.check_expr(operand, false)?;
                        }
                        Ok(())
                    }
                    _ => {
                        self.check_expr(operand, false)?;
                        Ok(())
                    }
                }
            }

            Expr::Cmp(left, _, right, _) => {
                self.check_expr(left, false)?;
                self.check_expr(right, false)?;
                Ok(())
            }

            Expr::Logical(left, _, right, _) => {
                self.check_expr(left, false)?;
                self.check_expr(right, false)?;
                Ok(())
            }

            Expr::Call(callee, args, span) => {
                let fn_name = match callee.as_ref() {
                    Expr::Ident(n, _) => Some(n.clone()),
                    _ => None,
                };

                let param_info: Vec<(bool, bool)> = match fn_name.as_ref() {
                    Some(name) => {
                        self.fn_sigs.get(name.as_str()).map(|params| {
                            params.iter().map(|(_, ty)| {
                                (Self::is_ref_type(ty), matches!(ty, Type::MutRef(_)))
                            }).collect()
                        }).unwrap_or_default()
                    }
                    None => vec![],
                };

                for (i, arg) in args.iter().enumerate() {
                    let (is_ref_param, is_mut_param) = param_info.get(i).copied().unwrap_or((false, false));

                    if is_ref_param {
                        if let Expr::Ident(var, _) = arg {
                            let holder = format!("_arg_{}_{}", i, self.scope_stack.len());
                            self.track_binding(&holder);
                            if is_mut_param {
                                self.add_mut_borrow(var, &holder, span);
                            } else {
                                self.add_imm_borrow(var, &holder, span);
                            }
                        }
                        self.check_expr(arg, false)?;
                    } else {
                        self.check_expr(arg, false)?;
                        if let Expr::Ident(var, _) = arg {
                            if let Some(ty) = self.var_types.get(var) {
                                if !Self::is_copy_type(ty) && !Self::is_ref_type(ty) {
                                    self.mark_moved(var, span);
                                }
                            }
                        }
                    }
                }
                Ok(())
            }

            Expr::MethodCall(obj, method, args, span) => {
                let obj_is_ident = if let Expr::Ident(v, _) = obj.as_ref() { Some(v.clone()) } else { None };

                let obj_type = obj_is_ident.as_ref()
                    .and_then(|v| self.var_types.get(v))
                    .map(|t| t.display());

                let method_key = obj_type.as_ref()
                    .map(|tn| (tn.clone(), method.clone()));

                let all_param_info: Vec<(bool, bool)> = match method_key.as_ref() {
                    Some(key) => {
                        self.method_sigs.get(key).map(|params| {
                            params.iter().map(|(_, ty)| {
                                (Self::is_ref_type(ty), matches!(ty, Type::MutRef(_)))
                            }).collect()
                        }).unwrap_or_default()
                    }
                    None => vec![],
                };

                let self_info = all_param_info.first().copied();

                if let Some(ref var) = obj_is_ident {
                    match self_info {
                        Some((true, true)) => {
                            let holder = format!("_self_{}", self.scope_stack.len());
                            self.track_binding(&holder);
                            self.add_mut_borrow(var, &holder, span);
                        }
                        Some((true, false)) => {
                            let holder = format!("_self_{}", self.scope_stack.len());
                            self.track_binding(&holder);
                            self.add_imm_borrow(var, &holder, span);
                        }
                        Some((false, _)) => {
                            if let Some(ty) = self.var_types.get(var) {
                                if !Self::is_copy_type(ty) && !Self::is_ref_type(ty) {
                                    self.mark_moved(var, span);
                                }
                            }
                        }
                        None => {}
                    }
                } else {
                    self.check_expr(obj, false)?;
                }

                for (i, arg) in args.iter().enumerate() {
                    let offset = if obj_is_ident.is_some() { 1 } else { 0 };
                    let (is_ref_param, is_mut_param) = all_param_info.get(i + offset).copied().unwrap_or((false, false));

                    if is_ref_param {
                        if let Expr::Ident(var, _) = arg {
                            let holder = format!("_marg_{}_{}", i, self.scope_stack.len());
                            self.track_binding(&holder);
                            if is_mut_param {
                                self.add_mut_borrow(var, &holder, span);
                            } else {
                                self.add_imm_borrow(var, &holder, span);
                            }
                        }
                        self.check_expr(arg, false)?;
                    } else {
                        self.check_expr(arg, false)?;
                        if let Expr::Ident(var, _) = arg {
                            if let Some(ty) = self.var_types.get(var) {
                                if !Self::is_copy_type(ty) && !Self::is_ref_type(ty) {
                                    self.mark_moved(var, span);
                                }
                            }
                        }
                    }
                }
                Ok(())
            }

            Expr::Index(obj, idx, _) => {
                self.check_expr(obj, false)?;
                self.check_expr(idx, false)?;
                Ok(())
            }

            Expr::Attribute(obj, _, _) => {
                self.check_expr(obj, false)?;
                Ok(())
            }

            Expr::If(cond, body, elifs, else_body, _) => {
                self.check_expr(cond, false)?;
                self.enter_scope();
                for s in body { self.check_stmt(s)?; }
                self.exit_scope();

                for (ec, eb) in elifs {
                    self.check_expr(ec, false)?;
                    self.enter_scope();
                    for s in eb { self.check_stmt(s)?; }
                    self.exit_scope();
                }

                if let Some(eb) = else_body {
                    self.enter_scope();
                    for s in eb { self.check_stmt(s)?; }
                    self.exit_scope();
                }
                Ok(())
            }

            Expr::Match(value, arms, _) => {
                self.check_expr(value, false)?;
                for (_, body) in arms {
                    self.enter_scope();
                    for s in body { self.check_stmt(s)?; }
                    self.exit_scope();
                }
                Ok(())
            }

            Expr::ForLoop(var, iterable, body, _) => {
                self.check_expr(iterable, false)?;
                self.var_types.insert(var.clone(), Type::Int);
                self.enter_scope();
                for s in body { self.check_stmt(s)?; }
                self.exit_scope();
                Ok(())
            }

            Expr::WhileLoop(cond, body, _) => {
                self.check_expr(cond, false)?;
                self.enter_scope();
                for s in body { self.check_stmt(s)?; }
                self.exit_scope();
                Ok(())
            }

            Expr::Block(stmts, _) => {
                self.enter_scope();
                for s in stmts {
                    self.check_stmt(s)?;
                }
                self.exit_scope();
                Ok(())
            }

            Expr::Lambda(_, _, _) => Ok(()),

            Expr::StructLit(_, fields, _) => {
                for (_, fv) in fields {
                    self.check_expr(fv, false)?;
                }
                Ok(())
            }

            Expr::EnumVariant(_, _, args, _) => {
                for a in args { self.check_expr(a, false)?; }
                Ok(())
            }

            Expr::List(items, _) => {
                for i in items { self.check_expr(i, false)?; }
                Ok(())
            }

            Expr::Tuple(items, _) => {
                for i in items { self.check_expr(i, false)?; }
                Ok(())
            }

            Expr::Await(inner, span) => {
                self.check_expr(inner, false)?;
                if let Expr::Ident(var, _) = inner.as_ref() {
                    if let Some(ty) = self.var_types.get(var) {
                        if !Self::is_copy_type(ty) {
                            self.mark_moved(var, span);
                        }
                    }
                }
                Ok(())
            }

            Expr::Spawn(inner, span) => {
                if let Expr::Call(_, args, _) = inner.as_ref() {
                    for arg in args {
                        self.check_expr(arg, false)?;
                        if let Expr::Ident(var, _) = arg {
                            if let Some(ty) = self.var_types.get(var) {
                                if !Self::is_copy_type(ty) && !Self::is_ref_type(ty) {
                                    self.mark_moved(var, span);
                                }
                            }
                        }
                    }
                } else {
                    self.check_expr(inner, false)?;
                }
                Ok(())
            }

            Expr::Make(_, size, _) => {
                self.check_expr(size, false)?;
                Ok(())
            }

            Expr::Range(start, end, _) => {
                self.check_expr(start, false)?;
                self.check_expr(end, false)?;
                Ok(())
            }

            Expr::Assign(target, _, value, _) => {
                self.check_expr(value, false)?;
                self.check_expr(target, true)?;
                Ok(())
            }

            Expr::TryOp(inner, _) => {
                self.check_expr(inner, false)?;
                Ok(())
            }

            Expr::Cast(inner, _, _) => {
                self.check_expr(inner, false)?;
                Ok(())
            }
        }
    }
}
