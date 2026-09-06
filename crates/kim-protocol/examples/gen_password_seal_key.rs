fn main() {
    let key = kim_protocol::PasswordSealKey::generate(None);
    println!("KIM_AUTH_PASSWORD_SEAL_PRIVATE={}", key.private_key_b64());
    println!("KIM_AUTH_PASSWORD_SEAL_KEY_ID={}", key.key_id());
    println!("alg={}", key.alg());
    println!("public_key_b64={}", key.public_key_b64());
}
