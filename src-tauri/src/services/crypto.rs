use std::path::PathBuf;
use std::sync::OnceLock;

static FERNET_INSTANCE: OnceLock<fernet::Fernet> = OnceLock::new();

/// 获取密钥文件路径: ~/.local/share/inspection-rust/.key
fn key_file_path() -> Result<PathBuf, String> {
    let dir = dirs::data_dir()
        .ok_or_else(|| "无法获取数据目录".to_string())?
        .join("inspection-rust");
    Ok(dir.join(".key"))
}

/// 加载或创建 Fernet 密钥。
/// 首次启动时生成随机密钥并保存到文件（权限 0600），
/// 后续启动从文件读取。
fn load_or_create_key() -> Result<String, String> {
    let path = key_file_path()?;

    // 确保目录存在
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("创建密钥目录失败: {}", e))?;
    }

    if path.exists() {
        let key = std::fs::read_to_string(&path)
            .map_err(|e| format!("读取密钥文件失败: {}", e))?;
        let key = key.trim().to_string();
        if key.is_empty() {
            return Err("密钥文件为空".to_string());
        }
        Ok(key)
    } else {
        let key = fernet::Fernet::generate_key();
        std::fs::write(&path, &key)
            .map_err(|e| format!("写入密钥文件失败: {}", e))?;

        // 设置文件权限为 0600（仅 Unix）
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("设置密钥文件权限失败: {}", e))?;
        }

        Ok(key)
    }
}

/// 获取全局 Fernet 实例（懒加载单例）
fn get_fernet() -> Result<&'static fernet::Fernet, String> {
    let instance = FERNET_INSTANCE.get_or_init(|| {
        let key = load_or_create_key().expect("加载 Fernet 密钥失败");
        fernet::Fernet::new(&key).expect("无效的 Fernet 密钥")
    });
    Ok(instance)
}

pub struct CryptoService;

impl CryptoService {
    pub fn encrypt(plaintext: &str) -> Result<String, String> {
        let fernet = get_fernet()?;
        Ok(fernet.encrypt(plaintext.as_bytes()))
    }

    pub fn decrypt(encrypted: &str) -> Result<String, String> {
        let fernet = get_fernet()?;
        let bytes = fernet
            .decrypt(encrypted)
            .map_err(|e| format!("解密失败: {}", e))?;
        String::from_utf8(bytes).map_err(|_| "UTF-8 转换失败".to_string())
    }
}
