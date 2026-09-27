use struct_path::{paths, StructPath};

#[derive(StructPath)]
struct Child {
    value_str: String,
}

struct Other {
    value_num: u64,
}

struct Parent {
    child: Child,
}

fn main() {
    let _ = paths!(Parent::child.(Child::*), Other::value_num);
}
