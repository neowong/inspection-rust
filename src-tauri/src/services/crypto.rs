const MASTER_PASSWORD: &str = "2buJOnYoEeKAjm18AqfX6JT73VIKHI2UQnh-FQUS-bE=";

pub struct CryptoService;

impl CryptoService {
    pub fn encrypt(plaintext: &str) -> Result<String, String> {
        let key = fernet::Fernet::generate_key();
        let fernet = fernet::Fernet::new(&key).ok_or("Invalid Fernet key")?;
        Ok(fernet.encrypt(plaintext.as_bytes()).to_string())
    }

    pub fn decrypt(encrypted: &str) -> Result<String, String> {
        let fernet = fernet::Fernet::new(MASTER_PASSWORD)
            .ok_or("Invalid Fernet key")?;
        let bytes = fernet.decrypt(encrypted)
            .map_err(|e| format!("Decrypt failed: {}", e))?;
        Ok(String::from_utf8(bytes).unwrap_or_default())
    }
}
