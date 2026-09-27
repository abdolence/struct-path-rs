use struct_path::{paths, StructPath};

#[derive(StructPath)]
struct Child {
    value_str: String,
}

struct Other {
    x: String,
}

struct Parent {
    child: Child,
}

fn main() {
    let _ = paths!(Other::(x), Parent::child.(Child::*));
}
