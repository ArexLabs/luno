use crate::error::Span;

pub type Ident = String;

#[derive(Debug, Clone)]
pub enum Literal {
    Int(i64),
    Float(f64),
    Bool(bool),
    Char(u8),
    String(String),
    Null,
}

#[derive(Debug, Clone)]
pub enum UnaryOp {
    Neg,
    Not,
    Ref,     // &
    MutRef,  // &mut
    Deref,   // *
}

#[derive(Debug, Clone)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    BitAnd, BitOr, BitXor, Shl, Shr,
    And, Or,
    Concat,
}

#[derive(Debug, Clone)]
pub enum AssignOp {
    Eq,
    AddEq, SubEq, MulEq, DivEq,
}

#[derive(Debug, Clone)]
pub enum CmpOp {
    Eq, NotEq, Lt, Gt, LtEq, GtEq,
}

#[derive(Debug, Clone)]
pub enum TypeExpr {
    Named(Ident, Span),
    Generic(Ident, Vec<TypeExpr>, Span),  // List[Int]
    FnType(Vec<TypeExpr>, Option<Box<TypeExpr>>, Span),
    Ref(Box<TypeExpr>, Span),
    MutRef(Box<TypeExpr>, Span),
    Infer(Span),  // _
    Tuple(Vec<TypeExpr>, Span),
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard(Span),
    Literal(Literal, Span),
    Binding(Ident, Span),
    Variant(Ident, Vec<Pattern>, Span),
    StructPattern(Ident, Vec<(Ident, Pattern)>, Span),
    Tuple(Vec<Pattern>, Span),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal, Span),
    Ident(Ident, Span),
    BinOp(Box<Expr>, BinOp, Box<Expr>, Span),
    UnaryOp(UnaryOp, Box<Expr>, Span),
    Cmp(Box<Expr>, CmpOp, Box<Expr>, Span),
    Logical(Box<Expr>, BinOp, Box<Expr>, Span),
    Call(Box<Expr>, Vec<Expr>, Span),
    MethodCall(Box<Expr>, Ident, Vec<Expr>, Span),
    Index(Box<Expr>, Box<Expr>, Span),
    Attribute(Box<Expr>, Ident, Span),
    If(Box<Expr>, Vec<Stmt>, Vec<(Expr, Vec<Stmt>)>, Option<Vec<Stmt>>, Span),
    Match(Box<Expr>, Vec<(Pattern, Vec<Stmt>)>, Span),
    ForLoop(Ident, Box<Expr>, Vec<Stmt>, Span),
    WhileLoop(Box<Expr>, Vec<Stmt>, Span),
    Block(Vec<Stmt>, Span),
    Lambda(Vec<Param>, Box<Expr>, Span),
    StructLit(Ident, Vec<(Ident, Expr)>, Span),
    EnumVariant(Ident, Ident, Vec<Expr>, Span),
    List(Vec<Expr>, Span),
    Tuple(Vec<Expr>, Span),
    Await(Box<Expr>, Span),
    Spawn(Box<Expr>, Span),
    Make(TypeExpr, Box<Expr>, Span),
    Range(Box<Expr>, Box<Expr>, Span),
    Assign(Box<Expr>, AssignOp, Box<Expr>, Span),
    TryOp(Box<Expr>, Span),
    Cast(Box<Expr>, TypeExpr, Span),
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: Ident,
    pub type_hint: Option<TypeExpr>,
    pub default: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        name: Ident,
        type_hint: Option<TypeExpr>,
        value: Expr,
        mutable: bool,
        span: Span,
    },
    Const {
        name: Ident,
        value: Expr,
        span: Span,
    },
    Assign {
        target: Expr,
        value: Expr,
        span: Span,
    },
    Expr(Expr),
    Return(Option<Expr>, Span),
    Break(Span),
    Continue(Span),
    FnDef {
        name: Ident,
        params: Vec<Param>,
        return_type: Option<TypeExpr>,
        body: Vec<Stmt>,
        span: Span,
    },
    TypeDef {
        name: Ident,
        generics: Vec<Ident>,
        fields: Vec<Field>,
        span: Span,
    },
    EnumDef {
        name: Ident,
        generics: Vec<Ident>,
        variants: Vec<Variant>,
        span: Span,
    },
    ImplBlock {
        type_name: Ident,
        methods: Vec<Stmt>,
        span: Span,
    },
    ImplTrait {
        trait_name: Ident,
        type_name: Ident,
        methods: Vec<Stmt>,
        span: Span,
    },
    TraitDef {
        name: Ident,
        methods: Vec<TraitMethod>,
        span: Span,
    },
    Import {
        module: Ident,
        span: Span,
    },
    FromImport {
        module: Ident,
        names: Vec<Ident>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: Ident,
    pub type_expr: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: Ident,
    pub types: Vec<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TraitMethod {
    pub name: Ident,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub stmts: Vec<Stmt>,
}
