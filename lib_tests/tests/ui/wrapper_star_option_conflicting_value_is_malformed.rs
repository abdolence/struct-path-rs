use struct_path::{paths, StructPath};

#[derive(StructPath)]
struct TestStruct {
    value_str: String,
}

macro_rules! camel_paths {
    ($($x:tt)*) => { paths!($($x)*; case = "camel") };
}

fn main() {
    let _ = camel_paths!(TestStruct::*; case = "pascal");
}
