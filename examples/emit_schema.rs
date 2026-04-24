//! Emit the JSON Schema for ProfileDocument to stdout.
//!
//! Usage:
//!   cargo run --example emit_schema > docs/pidx-schema.json
//!
//! The schema is derived from the Rust types via schemars — it is always
//! consistent with the structs that actually serialize to disk. Regenerate
//! after any model change and commit the updated file.

fn scrub_timestamps(val: &mut serde_json::Value) {
    match val {
        serde_json::Value::Object(map) => {
            if let Some(def) = map.get_mut("default") {
                if let serde_json::Value::String(s) = def {
                    if s.contains("T") && (s.contains("+00:00") || s.contains("Z")) {
                        *s = "1970-01-01T00:00:00Z".to_string();
                    }
                }
            }
            for v in map.values_mut() {
                scrub_timestamps(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                scrub_timestamps(v);
            }
        }
        _ => {}
    }
}

fn main() {
    let schema = schemars::schema_for!(pidx::models::profile::ProfileDocument);
    let mut val = serde_json::to_value(&schema).unwrap();
    scrub_timestamps(&mut val);

    let s = serde_json::to_string_pretty(&val).expect("schema serialization failed");
    if std::env::args().any(|a| a == "--write") {
        std::fs::write("docs/pidx-schema.json", s + "\n").expect("failed to write file");
    } else {
        println!("{}", s);
    }
}
