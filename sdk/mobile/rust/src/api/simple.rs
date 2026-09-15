#[flutter_rust_bridge::frb(sync)] // Synchronous mode for simplicity of the demo
pub fn greet(name: String) -> String {
    format!("Hello, {name}!")
}

pub fn spec_json_to_blob(body_json: String) -> Result<Vec<u8>, String> {
    kim_agent_codec::json_to_blob(&body_json).map_err(|e| e.to_string())
}

pub fn spec_blob_to_json(blob: Vec<u8>) -> Result<String, String> {
    kim_agent_codec::blob_to_json(&blob).map_err(|e| e.to_string())
}

#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    // Default utilities - feel free to customize
    flutter_rust_bridge::setup_default_user_utils();
}
