use std::path::Path;

pub fn generate_id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("getrandom failed");
    hex::encode(bytes)
}

pub fn exists_check(id: &str, base_path: &Path) -> bool {
    let session_dir = base_path.join(id);
    session_dir.exists()
}
