use struct_path::{paths, StructPath};

#[derive(StructPath)]
struct Child {
    value_str: String,
}

struct Parent {
    a: String,
    b: String,
}

fn main() {
    let _ = paths!(Parent::{ a, b }.(Child::*));
}
