const MASTER_PASSWORD: &str = "2buJOnYoEeKAjm18AqfX6JT73VIKHI2UQnh-FQUS-bE=";

pub struct CryptoService;

impl CryptoService {
    pub fn encrypt(plaintext: &str) -> Result<String, String> {
        let fernet = fernet::Fernet::new(MASTER_PASSWORD)
            .ok_or_else(|| "Invalid Fernet key".to_string())?;
        Ok(fernet.encrypt(plaintext.as_bytes()).to_string())
    }

    pub fn decrypt(encrypted: &str) -> Result<String, String> {
        let fernet = fernet::Fernet::new(MASTER_PASSWORD)
            .ok_or_else(|| "Invalid Fernet key".to_string())?;
        let bytes = fernet
            .decrypt(encrypted)
            .map_err(|e| format!("解密失败: {}", e))?;
        String::from_utf8(bytes).map_err(|_| "UTF-8 转换失败".to_string())
    }
}
