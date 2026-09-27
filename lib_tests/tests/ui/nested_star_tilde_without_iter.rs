use struct_path::{paths, StructPath};

#[derive(StructPath)]
struct Child {
    value_str: String,
}

struct Parent {
    child: u64,
}

fn main() {
    let _ = paths!(Parent::child~(Child::*));
}
