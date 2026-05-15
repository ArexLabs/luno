use crate::types::Type;

pub fn get_builtin_types() -> Vec<(String, Type)> {
    vec![
        ("Result".into(), Type::Generic("Result".into(), vec![Type::TypeParam(0), Type::TypeParam(1)])),
        ("Option".into(), Type::Generic("Option".into(), vec![Type::TypeParam(0)])),
        ("Future".into(), Type::Generic("Future".into(), vec![Type::TypeParam(0)])),
        ("Chan".into(), Type::Generic("Chan".into(), vec![Type::TypeParam(0)])),
    ]
}

pub fn get_builtin_enums() -> Vec<(String, Vec<(String, Vec<Type>)>, Vec<String>)> {
    vec![
        ("Result".into(), vec![
            ("Ok".into(), vec![Type::TypeParam(0)]),
            ("Err".into(), vec![Type::TypeParam(1)]),
        ], vec!["T".into(), "E".into()]),
        ("Option".into(), vec![
            ("Some".into(), vec![Type::TypeParam(0)]),
            ("None".into(), vec![]),
        ], vec!["T".into()]),
    ]
}
