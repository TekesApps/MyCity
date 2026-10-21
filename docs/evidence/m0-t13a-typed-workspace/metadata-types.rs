// Measurement of public Rust layouts only; private BTree bounds come from the
// pinned local Rust 1.85.1 library source, not unsafe layout introspection.
fn main() {
    println!("{{\"pointer\":{},\"string\":{},\"value\":{},\"vec_u8\":{},\"arc_vec_u8\":{},\"mutex_unit\":{}}}",
        std::mem::size_of::<usize>(),
        std::mem::size_of::<String>(),
        std::mem::size_of::<serde_json::Value>(),
        std::mem::size_of::<Vec<u8>>(),
        std::mem::size_of::<std::sync::Arc<Vec<u8>>>(),
        std::mem::size_of::<std::sync::Mutex<()>>());
}
