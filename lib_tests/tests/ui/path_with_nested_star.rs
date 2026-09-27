use struct_path::{path, StructPath};

#[derive(StructPath)]
struct Child {
    value_str: String,
}

struct Parent {
    child: Child,
}

fn main() {
    let _ = path!(Parent::child.(Child::*));
}
