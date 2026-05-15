use std::collections::HashMap;
use crate::ast::*;
use crate::error::{Diag, Diagnostics};
use crate::types::*;
use crate::builtins;

pub struct Checker {
    pub types: TypeTable,
    pub vars: HashMap<String, Type>,
    pub diags: Diagnostics,
    pub fn_ret_type: Option<Type>,
    pub generic_map: HashMap<String, Type>,
}

impl Checker {
    pub fn new() -> Self {
        let mut checker = Checker {
            types: TypeTable::new(),
            vars: HashMap::new(),
            diags: Diagnostics::new(),
            fn_ret_type: None,
            generic_map: HashMap::new(),
        };

        for (name, variants, params) in builtins::get_builtin_enums() {
            checker.types.add_enum(&name, variants, params);
        }

        checker
    }

    pub fn check_program(&mut self, program: &Program) {
        for stmt in &program.stmts {
            match stmt {
                Stmt::TypeDef { name, generics, fields, span: _ } => {
                    let field_types: Vec<(String, Type)> = fields.iter()
                        .map(|f| (f.name.clone(), self.type_expr_to_type(&f.type_expr)))
                        .collect();
                    self.types.add_struct(name, field_types, generics.clone());
                }
                Stmt::EnumDef { name, generics, variants, span: _ } => {
                    let variant_types: Vec<(String, Vec<Type>)> = variants.iter()
                        .map(|v| (v.name.clone(), v.types.iter().map(|t| self.type_expr_to_type(t)).collect()))
                        .collect();
                    self.types.add_enum(name, variant_types, generics.clone());
                }
                Stmt::FnDef { name: _, params, return_type, body, span: _ } => {
                    for param in params {
                        if let Some(ref t) = param.type_hint {
                            let ptype = self.type_expr_to_type(t);
                            self.vars.insert(param.name.clone(), ptype);
                        }
                    }

                    let ret = return_type.as_ref().map(|t| self.type_expr_to_type(t));
                    self.fn_ret_type = ret.clone();

                    for stmt in body {
                        self.check_stmt(stmt);
                    }
                }
                Stmt::ImplBlock { type_name, methods, span: _ } => {
                    let mut sigs = Vec::new();
                    for method in methods {
                        if let Stmt::FnDef { name, params, return_type, .. } = method {
                            let ptypes: Vec<(String, Type)> = params.iter()
                                .filter_map(|p| {
                                    p.type_hint.as_ref().map(|t| (p.name.clone(), self.type_expr_to_type(t)))
                                })
                                .collect();
                            let ret = return_type.as_ref().map(|t| self.type_expr_to_type(t));
                            sigs.push(FnSig {
                                name: name.clone(),
                                params: ptypes,
                                return_type: ret,
                            });
                        }
                    }
                    self.types.methods.insert(type_name.clone(), sigs);
                }
                Stmt::ImplTrait { trait_name, type_name, methods, span: _ } => {
                    let mut sigs = Vec::new();
                    for method in methods {
                        if let Stmt::FnDef { name, params, return_type, .. } = method {
                            let ptypes: Vec<(String, Type)> = params.iter()
                                .filter_map(|p| p.type_hint.as_ref().map(|t| (p.name.clone(), self.type_expr_to_type(t))))
                                .collect();
                            let ret = return_type.as_ref().map(|t| self.type_expr_to_type(t));
                            sigs.push(FnSig {
                                name: name.clone(),
                                params: ptypes,
                                return_type: ret,
                            });
                        }
                    }
                    self.types.impl_traits.insert((trait_name.clone(), type_name.clone()), sigs);
                }
                Stmt::TraitDef { name, methods, span: _ } => {
                    let mut sigs = Vec::new();
                    for m in methods {
                        let ptypes: Vec<(String, Type)> = m.params.iter()
                            .filter_map(|p| p.type_hint.as_ref().map(|t| (p.name.clone(), self.type_expr_to_type(t))))
                            .collect();
                        let ret = m.return_type.as_ref().map(|t| self.type_expr_to_type(t));
                        sigs.push(FnSig {
                            name: m.name.clone(),
                            params: ptypes,
                            return_type: ret,
                        });
                    }
                    self.types.traits.insert(name.clone(), sigs);
                }
                _ => {
                    self.check_stmt(stmt);
                }
            }
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Option<Type> {
        match stmt {
            Stmt::Let { name, type_hint, value, mutable: _, span } => {
                let val_type = self.check_expr(value);
                if let Some(ref hint) = type_hint {
                    let hint_type = self.type_expr_to_type(hint);
                    if val_type != hint_type {
                        self.diags.push(Diag::error(
                            format!("type mismatch: expected {}, got {}", hint_type.display(), val_type.display()),
                            span.clone(),
                        ));
                    }
                }
                self.vars.insert(name.clone(), val_type.clone());
                Some(val_type)
            }
            Stmt::Const { name, value, span: _ } => {
                let val_type = self.check_expr(value);
                self.vars.insert(name.clone(), val_type.clone());
                Some(val_type)
            }
            Stmt::Assign { target, value, span: _ } => {
                let val_type = self.check_expr(value);
                self.check_expr(target);
                Some(val_type)
            }
            Stmt::Expr(expr) => {
                Some(self.check_expr(expr))
            }
            Stmt::Return(expr, span) => {
                let expr_type = expr.as_ref().map(|e| self.check_expr(e)).unwrap_or(Type::Void);
                if let Some(ref ret) = self.fn_ret_type {
                    if expr_type != *ret && expr_type != Type::Void {
                        self.diags.push(Diag::error(
                            format!("type mismatch: expected return {}, got {}", ret.display(), expr_type.display()),
                            span.clone(),
                        ));
                    }
                }
                Some(expr_type)
            }
            Stmt::Break(_) | Stmt::Continue(_) => Some(Type::Void),
            Stmt::FnDef { name: _, params, return_type, body, span: _ } => {
                let ret = return_type.as_ref().map(|t| self.type_expr_to_type(t)).unwrap_or(Type::Void);
                self.fn_ret_type = Some(ret.clone());
                let saved = self.vars.clone();
                for param in params {
                    if let Some(ref t) = param.type_hint {
                        self.vars.insert(param.name.clone(), self.type_expr_to_type(t));
                    }
                }
                for s in body {
                    self.check_stmt(s);
                }
                self.vars = saved;
                Some(ret)
            }
            Stmt::TypeDef { .. } | Stmt::EnumDef { .. } | Stmt::ImplBlock { .. }
            | Stmt::ImplTrait { .. } | Stmt::TraitDef { .. } => None,
            Stmt::Import { .. } | Stmt::FromImport { .. } => None,
        }
    }

    pub fn check_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Literal(lit, _) => match lit {
                Literal::Int(_) => Type::Int,
                Literal::Float(_) => Type::Float,
                Literal::Bool(_) => Type::Bool,
                Literal::Char(_) => Type::Char,
                Literal::String(_) => Type::String,
                Literal::Null => Type::Never,
            }
            Expr::Ident(name, _) => {
                if name == "true" { return Type::Bool; }
                if name == "false" { return Type::Bool; }
                if let Some(t) = self.vars.get(name) {
                    t.clone()
                } else if let Some(t) = self.types.types.get(name) {
                    t.clone()
                } else {
                    // Type variable (unknown)
                    Type::Named(name.clone())
                }
            }
            Expr::BinOp(left, _, right, _) => {
                let lt = self.check_expr(left);
                let rt = self.check_expr(right);
                if lt == Type::Float || rt == Type::Float { Type::Float }
                else { lt }
            }
            Expr::UnaryOp(_op, operand, _) => {
                self.check_expr(operand)
            }
            Expr::Cmp(left, _, right, _) => {
                self.check_expr(left);
                self.check_expr(right);
                Type::Bool
            }
            Expr::Logical(left, _, right, _) => {
                self.check_expr(left);
                self.check_expr(right);
                Type::Bool
            }
            Expr::Call(callee, args, _) => {
                let callee_type = self.check_expr(callee);
                for arg in args {
                    self.check_expr(arg);
                }
                match callee_type {
                    Type::Fn(_, ret) => ret.map(|t| *t).unwrap_or(Type::Void),
                    _ => Type::Void,
                }
            }
            Expr::MethodCall(obj, method, args, _) => {
                let obj_type = self.check_expr(obj);
                for arg in args {
                    self.check_expr(arg);
                }
                if let Some(methods) = self.types.methods.get(&obj_type.display()) {
                    for msig in methods {
                        if msig.name == *method {
                            return msig.return_type.clone().unwrap_or(Type::Void);
                        }
                    }
                }

                let string_builtins: Vec<&str> = vec!["length", "upper", "lower", "split", "trim", "contains", "startsWith", "endsWith"];
                if string_builtins.contains(&method.as_str()) {
                    if *method == "length" { return Type::Int; }
                    if *method == "contains" || *method == "startsWith" || *method == "endsWith" { return Type::Bool; }
                    return Type::String;
                }

                let math_builtins: Vec<&str> = vec!["sqrt", "abs", "sin", "cos", "floor", "ceil"];
                if math_builtins.contains(&method.as_str()) {
                    return Type::Float;
                }

                Type::Void
            }
            Expr::Index(obj, idx, _) => {
                let obj_type = self.check_expr(obj);
                self.check_expr(idx);
                match obj_type {
                    Type::Slice(t) => *t,
                    Type::String => Type::Char,
                    _ => Type::Void,
                }
            }
            Expr::Attribute(obj, name, _) => {
                let obj_type = self.check_expr(obj);
                if let Type::Struct(_, fields, _) = &obj_type {
                    for (fname, ftype) in fields {
                        if fname == name {
                            return ftype.clone();
                        }
                    }
                }
                Type::Void
            }
            Expr::If(cond, body, elifs, else_body, _) => {
                self.check_expr(cond);
                let mut result = Type::Void;
                for s in body {
                    if let Some(t) = self.check_stmt(s) {
                        result = t;
                    }
                }
                for (_, eb) in elifs {
                    for s in eb {
                        if let Some(t) = self.check_stmt(s) {
                            result = t;
                        }
                    }
                }
                if let Some(eb) = else_body {
                    for s in eb {
                        if let Some(t) = self.check_stmt(s) {
                            result = t;
                        }
                    }
                }
                result
            }
            Expr::Match(value, arms, _) => {
                let val_type = self.check_expr(value);
                for (_, body) in arms {
                    for s in body {
                        self.check_stmt(s);
                    }
                }
                val_type
            }
            Expr::ForLoop(var, iterable, body, _) => {
                let _it_type = self.check_expr(iterable);
                self.vars.insert(var.clone(), Type::Int);
                for s in body {
                    self.check_stmt(s);
                }
                Type::Void
            }
            Expr::WhileLoop(cond, body, _) => {
                self.check_expr(cond);
                for s in body {
                    self.check_stmt(s);
                }
                Type::Void
            }
            Expr::Block(stmts, _) => {
                let mut result = Type::Void;
                for s in stmts {
                    if let Some(t) = self.check_stmt(s) {
                        result = t;
                    }
                }
                result
            }
            Expr::Lambda(_params, _body, _) => {
                Type::Fn(vec![], None)
            }
            Expr::StructLit(name, _fields, _) => {
                Type::Named(name.clone())
            }
            Expr::EnumVariant(enum_name, _variant, args, _) => {
                for arg in args {
                    self.check_expr(arg);
                }
                Type::Named(enum_name.clone())
            }
            Expr::List(items, _) => {
                if items.is_empty() {
                    Type::Slice(Box::new(Type::Never))
                } else {
                    let item_type = self.check_expr(&items[0]);
                    Type::Slice(Box::new(item_type))
                }
            }
            Expr::Tuple(items, _) => {
                let types: Vec<Type> = items.iter().map(|i| self.check_expr(i)).collect();
                Type::Tuple(types)
            }
            Expr::Await(inner, span) => {
                let inner_type = self.check_expr(inner);
                match inner_type {
                    Type::Future(t) => *t,
                    Type::Generic(name, args) if name == "Future" => {
                        args.into_iter().next().unwrap_or(Type::Never)
                    }
                    _ => {
                        self.diags.push(Diag::error(
                            format!("cannot await '{}': not a Future", inner_type.display()),
                            span.clone(),
                        ));
                        Type::Never
                    }
                }
            }
            Expr::Spawn(inner, _) => {
                let inner_type = self.check_expr(inner);
                Type::Future(Box::new(inner_type))
            }
            Expr::Make(chan_type, size, _) => {
                let _size_type = self.check_expr(size);
                let resolved = self.type_expr_to_type(chan_type);
                resolved
            }
            Expr::Range(start, end, _) => {
                self.check_expr(start);
                self.check_expr(end);
                Type::Slice(Box::new(Type::Int))
            }
            Expr::Assign(target, _op, value, _) => {
                self.check_expr(target);
                self.check_expr(value);
                Type::Void
            }
            Expr::TryOp(inner, _) => {
                let inner_type = self.check_expr(inner);
                match inner_type {
                    Type::Generic(name, args) if name == "Result" => {
                        args.into_iter().next().unwrap_or(Type::Void)
                    }
                    _ => inner_type
                }
            }
            Expr::Cast(inner, target_type, _) => {
                self.check_expr(inner);
                self.type_expr_to_type(target_type)
            }
        }
    }

    fn type_expr_to_type(&self, texpr: &TypeExpr) -> Type {
        match texpr {
            TypeExpr::Named(name, _) => {
                if let Some(t) = self.types.types.get(name) {
                    t.clone()
                } else {
                    Type::Named(name.clone())
                }
            }
            TypeExpr::Generic(name, args, _) => {
                let resolved: Vec<Type> = args.iter().map(|a| self.type_expr_to_type(a)).collect();
                Type::Generic(name.clone(), resolved)
            }
            TypeExpr::FnType(params, ret, _) => {
                let p: Vec<Type> = params.iter().map(|p| self.type_expr_to_type(p)).collect();
                Type::Fn(p, ret.as_ref().map(|r| Box::new(self.type_expr_to_type(r))))
            }
            TypeExpr::Ref(inner, _) => Type::Ref(Box::new(self.type_expr_to_type(inner))),
            TypeExpr::MutRef(inner, _) => Type::MutRef(Box::new(self.type_expr_to_type(inner))),
            TypeExpr::Infer(_) => Type::Never,
            TypeExpr::Tuple(ts, _) => {
                Type::Tuple(ts.iter().map(|t| self.type_expr_to_type(t)).collect())
            }
        }
    }
}
