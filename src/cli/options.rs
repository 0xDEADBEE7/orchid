use std::collections::BTreeMap;

pub(crate) fn extract(args: &[String]) -> (Vec<String>, BTreeMap<String, Option<String>>) {
    let mut filtered = Vec::with_capacity(args.len());
    let mut globals = BTreeMap::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--config" {
            if let Some(value) = args.get(i + 1) {
                globals.insert("config".to_string(), Some(value.clone()));
                i += 2;
                continue;
            }
        } else if let Some(value) = args[i].strip_prefix("--config=") {
            globals.insert("config".to_string(), Some(value.to_string()));
            i += 1;
            continue;
        }
        filtered.push(args[i].clone());
        i += 1;
    }
    (filtered, globals)
}
