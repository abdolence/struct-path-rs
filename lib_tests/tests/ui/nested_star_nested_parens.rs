use struct_path::{paths, StructPath};

#[derive(StructPath)]
struct B {
    value_str: String,
}

struct A {
    b: B,
}

struct Parent {
    a: A,
}

fn main() {
    let _ = paths!(Parent::a.(A::b.(B::*)));
}
