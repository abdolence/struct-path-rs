use struct_path::{paths, StructPath};

#[derive(StructPath)]
struct TestStruct {
    value_str: String,
}

macro_rules! all_fields_of {
    ($t:ty) => {
        paths!($t::*)
    };
}

fn main() {
    let _ = all_fields_of!(TestStruct);
}
