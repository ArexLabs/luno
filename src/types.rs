use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    Bool,
    Char,
    Byte,
    String,
    Never,
    Void,
    TypeParam(usize),          // generic parameter by index
    Named(String),             // user defined type
    Fn(Vec<Type>, Option<Box<Type>>),
    Ref(Box<Type>),
    MutRef(Box<Type>),
    Generic(String, Vec<Type>),
    Struct(String, Vec<(String, Type)>, Vec<String>), // name, fields, generic params
    Enum(String, Vec<(String, Vec<Type>)>, Vec<String>), // name, variants, generic params
    Future(Box<Type>),
    Chan(Box<Type>),
    Tuple(Vec<Type>),
    Slice(Box<Type>),
    Error,
}

impl Type {
    pub fn is_copy(&self) -> bool {
        matches!(self, Type::Int | Type::Float | Type::Bool | Type::Char | Type::Byte)
    }

    pub fn display(&self) -> String {
        match self {
            Type::Int => "Int".into(),
            Type::Float => "Float".into(),
            Type::Bool => "Bool".into(),
            Type::Char => "Char".into(),
            Type::Byte => "Byte".into(),
            Type::String => "String".into(),
            Type::Never => "!".into(),
            Type::Void => "Void".into(),
            Type::TypeParam(i) => format!("T{}", i),
            Type::Named(s) => s.clone(),
            Type::Fn(params, ret) => {
                let ps: Vec<String> = params.iter().map(|p| p.display()).collect();
                match ret {
                    Some(r) if **r != Type::Void => format!("({}) -> {}", ps.join(", "), r.display()),
                    _ => format!("({})", ps.join(", ")),
                }
            }
            Type::Ref(t) => format!("&{}", t.display()),
            Type::MutRef(t) => format!("&mut {}", t.display()),
            Type::Generic(name, args) => {
                let args: Vec<String> = args.iter().map(|a| a.display()).collect();
                format!("{}[{}]", name, args.join(", "))
            }
            Type::Struct(name, _, _) => name.clone(),
            Type::Enum(name, _, _) => name.clone(),
            Type::Future(t) => format!("Future[{}]", t.display()),
            Type::Chan(t) => format!("Chan[{}]", t.display()),
            Type::Tuple(ts) => {
                let ts: Vec<String> = ts.iter().map(|t| t.display()).collect();
                format!("({})", ts.join(", "))
            }
            Type::Slice(t) => format!("[{}]", t.display()),
            Type::Error => "error".into(),
        }
    }
}

pub struct TypeTable {
    pub types: HashMap<String, Type>,
    pub structs: HashMap<String, (Vec<(String, Type)>, Vec<String>)>,
    pub enums: HashMap<String, (Vec<(String, Vec<Type>)>, Vec<String>)>,
    pub methods: HashMap<String, Vec<FnSig>>,
    pub traits: HashMap<String, Vec<FnSig>>,
    pub impl_traits: HashMap<(String, String), Vec<FnSig>>,
}

#[derive(Debug, Clone)]
pub struct FnSig {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub return_type: Option<Type>,
}

impl TypeTable {
    pub fn new() -> Self {
        let mut tt = TypeTable {
            types: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            methods: HashMap::new(),
            traits: HashMap::new(),
            impl_traits: HashMap::new(),
        };
        tt.types.insert("Int".into(), Type::Int);
        tt.types.insert("Float".into(), Type::Float);
        tt.types.insert("Bool".into(), Type::Bool);
        tt.types.insert("Char".into(), Type::Char);
        tt.types.insert("Byte".into(), Type::Byte);
        tt.types.insert("String".into(), Type::String);
        tt
    }

    pub fn resolve_type(&self, name: &str, generics: &[Type]) -> Option<Type> {
        if generics.is_empty() {
            self.types.get(name).cloned()
        } else {
            Some(Type::Generic(name.into(), generics.to_vec()))
        }
    }

    pub fn add_struct(&mut self, name: &str, fields: Vec<(String, Type)>, generic_params: Vec<String>) {
        let gen_count = generic_params.len();
        self.structs.insert(name.into(), (fields.clone(), generic_params));
        let t = if gen_count > 0 {
            let params: Vec<Type> = (0..gen_count).map(|i| Type::TypeParam(i)).collect();
            Type::Generic(name.into(), params)
        } else {
            Type::Named(name.into())
        };
        self.types.insert(name.into(), t);
    }

    pub fn add_enum(&mut self, name: &str, variants: Vec<(String, Vec<Type>)>, generic_params: Vec<String>) {
        let gen_count = generic_params.len();
        self.enums.insert(name.into(), (variants, generic_params));
        let t = if gen_count > 0 {
            let params: Vec<Type> = (0..gen_count).map(|i| Type::TypeParam(i)).collect();
            Type::Generic(name.into(), params)
        } else {
            Type::Named(name.into())
        };
        self.types.insert(name.into(), t);
    }
}
