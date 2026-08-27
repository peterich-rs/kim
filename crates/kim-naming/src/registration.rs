use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct DefaultRegistration {
    pub service_id: String,
    pub service_name: String,
    pub protocol: String,
    pub public_address: String,
    pub public_port: u16,
    pub tags: Vec<String>,
    pub meta: HashMap<String, String>,
}

impl DefaultRegistration {
    pub fn dial_url(&self) -> String {
        format!("{}:{}", self.public_address, self.public_port)
    }
}
