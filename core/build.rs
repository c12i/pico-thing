// Generate src/config.rs's contents from config.toml at the repo root (gitignored;
// see config.example.toml): Wi-Fi, device addresses and tokens.
use std::{env, fs, path::PathBuf};
use toml::{Table, Value};

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("..");
    let mut path = root.join("config.toml");
    if !path.exists() {
        println!("cargo:warning=config.toml not found, using config.example.toml");
        path = root.join("config.example.toml");
    }
    println!("cargo:rerun-if-changed={}", path.display());
    let text = fs::read_to_string(&path).unwrap();
    let c: Table = text.parse().unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    let s = |k: &str| c.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let f = |k: &str| match c.get(k).and_then(|v| v.as_float().or(v.as_integer().map(|i| i as f64))) {
        Some(v) => format!("Some({v:?}_f32)"),
        None => "None".into(),
    };
    let list = |items: Vec<(String, String)>| -> String {
        let body: Vec<String> = items.iter().map(|(a, b)| format!("({a:?}, {b:?})")).collect();
        format!("&[{}]", body.join(", "))
    };
    // A table of "key" = "value", in file order
    let table = |k: &str| -> String {
        let items = match c.get(k).and_then(Value::as_table) {
            Some(t) => t.iter().map(|(a, b)| (a.clone(), b.as_str().unwrap_or("").to_string())).collect(),
            None => vec![],
        };
        list(items)
    };
    let sonos: Vec<(String, String)> = c
        .get("sonos")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|sp| Some((sp.get("name")?.as_str()?.to_string(), sp.get("ip")?.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();
    let quotes: Vec<String> = c
        .get("productivity")
        .and_then(|p| p.get("quotes"))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(|q| format!("{q:?}")).collect())
        .unwrap_or_default();

    let code = format!(
        "pub const WIFI_SSID: &str = {:?};\n\
         pub const WIFI_PASSWORD: &str = {:?};\n\
         pub const LATITUDE: Option<f32> = {};\n\
         pub const LONGITUDE: Option<f32> = {};\n\
         pub const CITY: &str = {:?};\n\
         pub const FAHRENHEIT: bool = {};\n\
         /// (IP, name) for Yeelights\n\
         pub const LIGHT_NAMES: &[(&str, &str)] = {};\n\
         /// (name, IP) for Sonos speakers\n\
         pub const SONOS: &[(&str, &str)] = {};\n\
         /// (IP, 32-hex-char token) for Xiaomi miIO lights\n\
         pub const MIIO_TOKENS: &[(&str, &str)] = {};\n\
         pub const COMPANION: &str = {:?};\n\
         pub const COMPANION_TOKEN: &str = {:?};\n\
         /// Overrides the built-in quotes when not empty\n\
         pub const QUOTES: &[&str] = &[{}];\n",
        s("wifi_ssid"),
        s("wifi_password"),
        f("latitude"),
        f("longitude"),
        s("city"),
        s("units") == "F",
        table("lights"),
        list(sonos),
        table("miio_tokens"),
        s("companion"),
        s("companion_token"),
        quotes.join(", "),
    );
    let dest = PathBuf::from(env::var("OUT_DIR").unwrap()).join("config.rs");
    fs::write(dest, code).unwrap();
}
