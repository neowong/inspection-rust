use serde::Serialize;
use rand::Rng;
use cipher::{BlockEncrypt, KeyInit, generic_array::GenericArray};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct SnmpResult {
    pub oid: String,
    pub value: Option<String>,
    pub value_type: Option<String>,
    pub error: Option<String>,
    pub response_time_ms: u64,
    pub raw_hex: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AuthProtocol {
    MD5,
    SHA1,
    SHA256,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrivProtocol {
    None,
    DES,
    AES128,
}

impl std::str::FromStr for AuthProtocol {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s.to_uppercase().as_str() {
            "MD5" => Ok(AuthProtocol::MD5),
            "SHA1" | "SHA" => Ok(AuthProtocol::SHA1),
            "SHA256" | "SHA-256" => Ok(AuthProtocol::SHA256),
            "NONE" => Err("认证协议不能为 None（v3 需认证）".into()),
            _ => Err(format!("不支持的认证协议: {}", s)),
        }
    }
}
impl AuthProtocol {
    fn key_len(&self) -> usize {
        match self { AuthProtocol::MD5 => 16, AuthProtocol::SHA1 => 20, AuthProtocol::SHA256 => 32 }
    }
    fn mac_len(&self) -> usize { 12 }
}

impl std::str::FromStr for PrivProtocol {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s.to_uppercase().as_str() {
            "NONE" => Ok(PrivProtocol::None),
            "DES" => Ok(PrivProtocol::DES),
            "AES128" | "AES" | "AES-128" => Ok(PrivProtocol::AES128),
            _ => Err(format!("不支持的加密协议: {}", s)),
        }
    }
}
impl PrivProtocol {
    fn salt_len(&self) -> usize {
        // RFC 3414 (DES) / RFC 3826 (AES): privParameters (salt) 均为 8 字节
        match self { PrivProtocol::DES => 8, PrivProtocol::AES128 => 8, PrivProtocol::None => 0 }
    }
}

/// 按 RFC 3414 (DES) / RFC 3826 (AES128) 由 salt + 引擎参数构造真正的加密 IV。
/// DES:  preIV = privKey[8..16]，IV = preIV ⊕ salt（8 字节）
/// AES:  IV = salt(8) ‖ engineBoots(4 BE) ‖ engineTime(4 BE)（16 字节）
fn build_priv_iv(priv_key: &[u8], salt: &[u8], engine_boots: u32, engine_time: u32, proto: PrivProtocol) -> Vec<u8> {
    match proto {
        PrivProtocol::DES => {
            let pre_iv = &priv_key[8..16];
            pre_iv.iter().zip(salt.iter()).map(|(p, s)| p ^ s).collect()
        }
        PrivProtocol::AES128 => {
            let mut iv = salt.to_vec();
            iv.extend_from_slice(&engine_boots.to_be_bytes());
            iv.extend_from_slice(&engine_time.to_be_bytes());
            iv
        }
        PrivProtocol::None => vec![],
    }
}

// ============================================================================
// Minimal ASN.1 BER encoder
// ============================================================================

fn encode_length(len: usize) -> Vec<u8> {
    if len < 128 {
        vec![len as u8]
    } else {
        let mut bytes = Vec::new();
        let mut remaining = len;
        while remaining > 0 {
            bytes.push((remaining & 0xFF) as u8);
            remaining >>= 8;
        }
        bytes.reverse();
        let mut result = vec![0x80 | bytes.len() as u8];
        result.extend(bytes);
        result
    }
}

fn encode_integer(val: u32) -> Vec<u8> {
    let mut result = vec![0x02];
    if val == 0 {
        result.push(0x01);
        result.push(0x00);
    } else {
        let mut bytes = Vec::new();
        let mut v = val;
        while v > 0 {
            bytes.push((v & 0xFF) as u8);
            v >>= 8;
        }
        bytes.reverse();
        // Ensure sign bit is 0
        if !bytes.is_empty() && (bytes[0] & 0x80) != 0 {
            result.extend(encode_length(bytes.len() + 1));
            result.push(0x00);
        } else {
            result.extend(encode_length(bytes.len()));
        }
        result.extend(bytes);
    }
    result
}

fn encode_octet_string(data: &[u8]) -> Vec<u8> {
    let mut result = vec![0x04];
    result.extend(encode_length(data.len()));
    result.extend_from_slice(data);
    result
}

fn encode_null() -> Vec<u8> {
    vec![0x05, 0x00]
}

fn encode_oid(oid_str: &str) -> Result<Vec<u8>, String> {
    let components: Vec<u64> = oid_str
        .split('.')
        .map(|s| s.parse::<u64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| format!("无效的OID: {}", oid_str))?;

    if components.len() < 2 {
        return Err("OID至少需要2个组件".into());
    }

    let first = components[0].checked_mul(40).and_then(|v| v.checked_add(components[1]))
        .ok_or_else(|| format!("OID 首组件溢出: {}", oid_str))?;
    let mut sub_ids = vec![first];
    sub_ids.extend_from_slice(&components[2..]);

    let mut encoded = Vec::new();
    for &id in &sub_ids {
        if id < 128 {
            encoded.push(id as u8);
        } else {
            let mut parts = Vec::new();
            let mut v = id;
            parts.push((v & 0x7F) as u8);
            v >>= 7;
            while v > 0 {
                parts.push(((v & 0x7F) | 0x80) as u8);
                v >>= 7;
            }
            parts.reverse();
            encoded.extend(parts);
        }
    }

    let mut result = vec![0x06];
    result.extend(encode_length(encoded.len()));
    result.extend(encoded);
    Ok(result)
}

fn encode_sequence(contents: &[u8]) -> Vec<u8> {
    let mut result = vec![0x30];
    result.extend(encode_length(contents.len()));
    result.extend_from_slice(contents);
    result
}

fn tagged(tag: u8, contents: &[u8]) -> Vec<u8> {
    let mut result = vec![tag];
    result.extend(encode_length(contents.len()));
    result.extend_from_slice(contents);
    result
}

// ============================================================================
// BER decoder
// ============================================================================

fn decode_length(data: &[u8], pos: &mut usize) -> Result<usize, String> {
    if *pos >= data.len() { return Err("数据截断".into()); }
    let first = data[*pos];
    *pos += 1;
    if first < 128 {
        Ok(first as usize)
    } else {
        let num_bytes = (first & 0x7F) as usize;
        // 限制长度字节数防止整数溢出（RFC 上限 4 字节，放宽到 8 防御）
        if num_bytes > 8 { return Err("长度字段过长".into()); }
        if *pos + num_bytes > data.len() { return Err("长度字段截断".into()); }
        let mut len: usize = 0;
        for _ in 0..num_bytes {
            len = (len << 8) | data[*pos] as usize;
            *pos += 1;
        }
        Ok(len)
    }
}

fn decode_tlv(data: &[u8], pos: &mut usize) -> Result<(u8, Vec<u8>), String> {
    if *pos >= data.len() { return Err("数据截断".into()); }
    let tag = data[*pos];
    *pos += 1;
    let len = decode_length(data, pos)?;
    // 用 checked_sub 防止 *pos + len 加法溢出后绕过边界检查
    let available = data.len().checked_sub(*pos).ok_or("值截断")?;
    if len > available { return Err("值截断".into()); }
    let value = data[*pos..*pos + len].to_vec();
    *pos += len;
    Ok((tag, value))
}

fn decode_oid_value(data: &[u8]) -> String {
    if data.is_empty() { return String::new(); }
    let mut components = Vec::new();
    let first = data[0] as u64;
    components.push((first / 40).to_string());
    components.push((first % 40).to_string());
    let mut i = 1;
    while i < data.len() {
        let mut val: u64 = 0;
        let mut cont_bytes = 0u8;
        loop {
            if i >= data.len() { break; } // 防止越界
            val = (val << 7) | (data[i] as u64 & 0x7F);
            let done = data[i] & 0x80 == 0;
            i += 1;
            cont_bytes += 1;
            if cont_bytes > 10 { break; } // 防止无限续延
            if done { break; }
        }
        components.push(val.to_string());
    }
    components.join(".")
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ")
}

fn format_snmp_value(tag: u8, data: &[u8]) -> (String, String) {
    match tag {
        0x02 => {
            // 限制 INTEGER 最多 8 字节防止移位溢出
            let data = if data.len() > 8 { &data[..8] } else { data };
            let mut val: i64 = 0;
            let negative = !data.is_empty() && (data[0] & 0x80) != 0;
            for &b in data { val = (val << 8) | b as i64; }
            // 仅 <8 字节时需要补码修正；8 字节时 i64 已是正确补码
            if negative && data.len() < 8 { val -= 1i64 << (data.len() * 8); }
            ("INTEGER".to_string(), val.to_string())
        }
        0x04 => ("OCTET STRING".to_string(), String::from_utf8_lossy(data).to_string()),
        0x05 => ("NULL".to_string(), "(null)".to_string()),
        0x06 => ("OID".to_string(), decode_oid_value(data)),
        0x41 => ("Counter32".to_string(), fold_u64(data, 4)),
        0x42 => ("Gauge32".to_string(), fold_u64(data, 4)),
        0x43 => ("TimeTicks".to_string(), fold_u64(data, 4)),
        0x46 => ("Counter64".to_string(), fold_u64(data, 8)),
        0x40 => ("IpAddress".to_string(), String::from_utf8_lossy(data).to_string()),
        _ => ("未知".to_string(), hex_encode(data)),
    }
}

/// 将字节数组折叠为 u64，限制最大字节数防止溢出
fn fold_u64(data: &[u8], max_len: usize) -> String {
    let data = if data.len() > max_len { &data[..max_len] } else { data };
    data.iter().fold(0u64, |a, &b| (a << 8) | b as u64).to_string()
}

fn decode_u32_integer(data: &[u8]) -> u32 {
    let data = if data.len() > 4 { &data[..4] } else { data };
    data.iter().fold(0u32, |a, &b| (a << 8) | b as u32)
}

// ============================================================================
// Crypto: key derivation, HMAC, CFB
// ============================================================================

macro_rules! localize_key {
    ($password:expr, $engine_id:expr, $Hasher:ty) => {{
        use digest::Digest;
        // Ku = hash of 1MB repeated password
        let ku = {
            let mut hasher = <$Hasher>::new();
            let mut remaining = 1_048_576usize;
            while remaining > 0 {
                let chunk = remaining.min($password.len());
                // 空密码会导致 chunk=0 → remaining 永不递减 → 死循环
                if chunk == 0 { break; }
                Digest::update(&mut hasher, &$password[..chunk]);
                remaining -= chunk;
            }
            Digest::finalize(hasher)
        };
        // Kul = hash(Ku || engineID || Ku)
        let mut h = <$Hasher>::new();
        Digest::update(&mut h, &ku);
        Digest::update(&mut h, $engine_id);
        Digest::update(&mut h, &ku);
        Digest::finalize(h).to_vec()
    }};
}

fn localize_auth_key(password: &[u8], engine_id: &[u8], proto: AuthProtocol) -> Vec<u8> {
    let kul = match proto {
        AuthProtocol::MD5 => localize_key!(password, engine_id, md5::Md5),
        AuthProtocol::SHA1 => localize_key!(password, engine_id, sha1::Sha1),
        AuthProtocol::SHA256 => localize_key!(password, engine_id, sha2::Sha256),
    };
    kul[..proto.key_len()].to_vec()
}

fn localize_priv_key(password: &[u8], engine_id: &[u8], priv_proto: PrivProtocol, auth_proto: AuthProtocol) -> Vec<u8> {
    let kul = match auth_proto {
        AuthProtocol::MD5 => localize_key!(password, engine_id, md5::Md5),
        AuthProtocol::SHA1 => localize_key!(password, engine_id, sha1::Sha1),
        AuthProtocol::SHA256 => localize_key!(password, engine_id, sha2::Sha256),
    };
    match priv_proto {
        // DES 需要 16 字节本地化密钥（8 字节密钥 + 8 字节 pre-IV）
        PrivProtocol::DES => {
            if kul.len() >= 16 { kul[..16].to_vec() }
            else { kul.clone() }
        }
        PrivProtocol::AES128 => kul[..16].to_vec(),
        PrivProtocol::None => vec![],
    }
}

fn compute_hmac(key: &[u8], data: &[u8], proto: AuthProtocol) -> Vec<u8> {
    use hmac::Mac;
    match proto {
        AuthProtocol::MD5 => {
            let mut mac = <hmac::Hmac<md5::Md5> as Mac>::new_from_slice(key).unwrap();
            mac.update(data);
            mac.finalize().into_bytes()[..12].to_vec()
        }
        AuthProtocol::SHA1 => {
            let mut mac = <hmac::Hmac<sha1::Sha1> as Mac>::new_from_slice(key).unwrap();
            mac.update(data);
            mac.finalize().into_bytes()[..12].to_vec()
        }
        AuthProtocol::SHA256 => {
            let mut mac = <hmac::Hmac<sha2::Sha256> as Mac>::new_from_slice(key).unwrap();
            mac.update(data);
            mac.finalize().into_bytes()[..12].to_vec()
        }
    }
}

fn encrypt_block(block: &mut [u8], key: &[u8], proto: PrivProtocol) {
    match proto {
        PrivProtocol::DES => {
            let cipher = des::Des::new(GenericArray::from_slice(key));
            let mut b = GenericArray::clone_from_slice(block);
            BlockEncrypt::encrypt_block(&cipher, &mut b);
            block.copy_from_slice(&b);
        }
        PrivProtocol::AES128 => {
            let cipher = aes::Aes128::new(GenericArray::from_slice(key));
            let mut b = GenericArray::clone_from_slice(block);
            BlockEncrypt::encrypt_block(&cipher, &mut b);
            block.copy_from_slice(&b);
        }
        PrivProtocol::None => {}
    }
}

fn cfb_encrypt(plaintext: &[u8], key: &[u8], iv: &[u8], proto: PrivProtocol) -> Vec<u8> {
    let block_size = iv.len(); // 8 for DES, 16 for AES
    let mut result = Vec::with_capacity(plaintext.len());
    let mut feedback = iv.to_vec();
    for chunk in plaintext.chunks(block_size) {
        let mut enc_feedback = feedback.clone();
        encrypt_block(&mut enc_feedback, key, proto);
        for (i, &p) in chunk.iter().enumerate() {
            result.push(p ^ enc_feedback[i]);
        }
        if chunk.len() == block_size {
            feedback.copy_from_slice(&result[result.len() - block_size..]);
        }
    }
    result
}

fn cfb_decrypt(ciphertext: &[u8], key: &[u8], iv: &[u8], proto: PrivProtocol) -> Vec<u8> {
    // CFB decryption uses encryption, same as encryption
    cfb_encrypt(ciphertext, key, iv, proto)
}

// ============================================================================
// SNMP v2c
// ============================================================================

pub fn build_snmp_v2c_get(community: &str, oid_str: &str) -> Result<Vec<u8>, String> {
    let oid_encoded = encode_oid(oid_str)?;
    let mut varbind = Vec::new();
    varbind.extend_from_slice(&oid_encoded);
    varbind.extend(encode_null());
    let wrapped_vb = encode_sequence(&varbind);
    let varbinds = encode_sequence(&wrapped_vb);

    let mut pdu_body = Vec::new();
    pdu_body.extend(encode_integer(1));   // request-id
    pdu_body.extend(encode_integer(0));   // error
    pdu_body.extend(encode_integer(0));   // error-index
    pdu_body.extend(&varbinds);

    let pdu = tagged(0xA0, &pdu_body);

    let mut snmp_body = Vec::new();
    snmp_body.extend(encode_integer(1));  // version = 1 (v2c)
    snmp_body.extend(encode_octet_string(community.as_bytes()));
    snmp_body.extend(&pdu);

    Ok(encode_sequence(&snmp_body))
}

pub async fn snmp_v2c_get(
    ip: &str,
    community: &str,
    oid: &str,
    timeout_secs: u64,
) -> Result<SnmpResult, String> {
    validate_ip(ip)?;
    let request_bytes = build_snmp_v2c_get(community, oid)?;
    snmp_send_and_parse_v2c(ip, &request_bytes, oid, timeout_secs).await
}

fn validate_ip(ip: &str) -> Result<(), String> {
    if ip.trim().is_empty() || ip.trim().parse::<std::net::IpAddr>().is_err() {
        Err("请输入有效的 IP 地址".into())
    } else {
        Ok(())
    }
}

async fn snmp_send_and_parse_v2c(
    ip: &str, request_bytes: &[u8], oid: &str, timeout_secs: u64,
) -> Result<SnmpResult, String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("无法创建UDP套接字: {}", e))?;
    socket.set_read_timeout(Some(std::time::Duration::from_secs(timeout_secs)))
        .map_err(|e| format!("无法设置超时: {}", e))?;

    let addr = format!("{}:161", ip);
    let start = std::time::Instant::now();
    socket.send_to(request_bytes, &addr)
        .map_err(|e| format!("发送SNMP请求失败: {}", e))?;

    let mut buf = [0u8; 4096];
    match socket.recv_from(&mut buf) {
        Ok((len, _src)) => {
            let elapsed = start.elapsed().as_millis() as u64;
            let response = &buf[..len];
            parse_v2c_response(response, oid, elapsed)
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
            Ok(SnmpResult {
                oid: oid.to_string(), value: None, value_type: None,
                error: Some("SNMP超时".into()),
                response_time_ms: start.elapsed().as_millis() as u64, raw_hex: None,
            })
        }
        Err(e) => Ok(SnmpResult {
            oid: oid.to_string(), value: None, value_type: None,
            error: Some(format!("接收响应失败: {}", e)),
            response_time_ms: start.elapsed().as_millis() as u64, raw_hex: None,
        }),
    }
}

fn parse_v2c_response(data: &[u8], requested_oid: &str, elapsed: u64) -> Result<SnmpResult, String> {
    let raw_hex = Some(hex_encode(data));
    let mut pos = 0;

    let (tag, seq_data) = decode_tlv(data, &mut pos)?;
    if tag != 0x30 { return Err(format!("期望SEQUENCE, 得到 0x{:02X}", tag)); }

    let mut inner = 0;
    let (tag, _ver) = decode_tlv(&seq_data, &mut inner)?;
    if tag != 0x02 { return Err(format!("期望INTEGER版本, 得到 0x{:02X}", tag)); }

    let (tag, _comm) = decode_tlv(&seq_data, &mut inner)?;
    if tag != 0x04 { return Err(format!("期望OCTET STRING community, 得到 0x{:02X}", tag)); }

    // PDU
    let (tag, pdu_data) = decode_tlv(&seq_data, &mut inner)?;
    if tag != 0xA2 { return Err(format!("期望GetResponse (0xA2), 得到 0x{:02X}", tag)); }

    let mut pp = 0;
    let (_, req_data) = decode_tlv(&pdu_data, &mut pp)?;
    let _req_id = decode_u32_integer(&req_data);
    let (_, err_data) = decode_tlv(&pdu_data, &mut pp)?;
    let error_status = decode_u32_integer(&err_data);

    if error_status != 0 {
        let err_msg = snmp_error_name(error_status);
        let (_, idx_data) = decode_tlv(&pdu_data, &mut pp)?;
        let error_index = decode_u32_integer(&idx_data);
        return Ok(SnmpResult {
            oid: requested_oid.to_string(), value: None, value_type: None,
            error: Some(format!("SNMP错误: {} (错误码: {}, 位置: {})", err_msg, error_status, error_index)),
            response_time_ms: elapsed, raw_hex,
        });
    }

    let (_, _idx_data) = decode_tlv(&pdu_data, &mut pp)?; // skip error-index
    // varbind list
    let (tag, vb_list) = decode_tlv(&pdu_data, &mut pp)?;
    if tag != 0x30 { return Err(format!("期望varbind列表, 得到 0x{:02X}", tag)); }

    let mut vp = 0;
    let (tag, vb_data) = decode_tlv(&vb_list, &mut vp)?;
    if tag != 0x30 { return Err(format!("期望varbind, 得到 0x{:02X}", tag)); }

    let mut vi = 0;
    let (tag, oid_data) = decode_tlv(&vb_data, &mut vi)?;
    if tag != 0x06 { return Err(format!("期望OID, 得到 0x{:02X}", tag)); }
    let _oid_str = decode_oid_value(&oid_data);

    let (tag, val_data) = decode_tlv(&vb_data, &mut vi)?;
    let (type_name, value_str) = format_snmp_value(tag, &val_data);

    Ok(SnmpResult {
        oid: requested_oid.to_string(),
        value: Some(value_str), value_type: Some(type_name),
        error: None, response_time_ms: elapsed, raw_hex,
    })
}

fn snmp_error_name(code: u32) -> &'static str {
    match code {
        1 => "tooBig", 2 => "noSuchName", 3 => "badValue", 4 => "readOnly",
        5 => "genErr", 6 => "noAccess", 7 => "wrongType", 8 => "wrongLength",
        9 => "wrongEncoding", 10 => "wrongValue", 11 => "noCreation",
        12 => "inconsistentValue", 13 => "resourceUnavailable", 14 => "commitFailed",
        15 => "undoFailed", 16 => "authorizationError", 17 => "notWritable",
        18 => "inconsistentName", _ => "未知错误",
    }
}

// ============================================================================
// SNMP v3
// ============================================================================

struct V3SecurityParams {
    engine_id: Vec<u8>,
    engine_boots: u32,
    engine_time: u32,
    username: Vec<u8>,
    auth_params: Vec<u8>,
    priv_params: Vec<u8>,
}

impl V3SecurityParams {
    fn encode(&self) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend(encode_octet_string(&self.engine_id));
        body.extend(encode_integer(self.engine_boots));
        body.extend(encode_integer(self.engine_time));
        body.extend(encode_octet_string(&self.username));
        body.extend(encode_octet_string(&self.auth_params));
        body.extend(encode_octet_string(&self.priv_params));
        encode_sequence(&body)
    }
}

struct V3ParsedResponse {
    engine_id: Vec<u8>,
    engine_boots: u32,
    engine_time: u32,
    scoped_pdu: Vec<u8>,   // if encrypted: raw OCTET STRING content; if not: full SEQUENCE TLV
    is_encrypted: bool,
}

#[allow(clippy::too_many_arguments)]
fn build_v3_message(
    engine_id: &[u8],
    engine_boots: u32,
    engine_time: u32,
    username: &str,
    auth_key: &[u8],
    priv_key: &[u8],
    auth_proto: AuthProtocol,
    priv_proto: PrivProtocol,
    oid: &str,
) -> Result<Vec<u8>, String> {
    let msg_id: u32 = rand::thread_rng().gen_range(0..0x7FFFFFFF);
    let msg_max_size: u32 = 65535;

    let reportable = true;
    let auth_flag = true;
    let priv_flag = priv_proto != PrivProtocol::None;
    let msg_flags: u8 = if reportable { 0x04 } else { 0x00 }
        | if auth_flag { 0x01 } else { 0x00 }
        | if priv_flag { 0x02 } else { 0x00 };
    let msg_security_model: u32 = 3; // USM

    // Build scoped PDU
    let scoped_pdu = build_scoped_pdu(engine_id, oid)?;

    // Build USM security params
    let auth_params_len = auth_proto.mac_len();
    let priv_params_len = if priv_flag { priv_proto.salt_len() } else { 0 };

    let usm = V3SecurityParams {
        engine_id: engine_id.to_vec(),
        engine_boots,
        engine_time,
        username: username.as_bytes().to_vec(),
        auth_params: vec![0u8; auth_params_len],
        priv_params: {
            if priv_flag {
                // RFC 3414/3826: salt = engineBoots(4 BE) ‖ randomInt(4)
                let mut salt = vec![0u8; priv_params_len];
                salt[..4].copy_from_slice(&engine_boots.to_be_bytes());
                rand::thread_rng().fill(&mut salt[4..]);
                salt
            } else {
                vec![]
            }
        },
    };
    let usm_encoded = usm.encode();

    // Build msgGlobalData
    let mut global_data = Vec::new();
    global_data.extend(encode_integer(msg_id));
    global_data.extend(encode_integer(msg_max_size));
    global_data.extend(encode_octet_string(&[msg_flags]));
    global_data.extend(encode_integer(msg_security_model));
    let global_data_enc = encode_sequence(&global_data);

    // Assemble whole message (without auth MAC)
    let mut whole_msg = Vec::new();
    whole_msg.extend(encode_integer(3)); // version = 3
    whole_msg.extend(&global_data_enc);
    whole_msg.extend(encode_octet_string(&usm_encoded));

    // Encrypt scoped PDU if needed; msgData is SEQUENCE (0x30) when unencrypted,
    // OCTET STRING (0x04) only when encryption is applied
    let msg_data_bytes: Vec<u8> = if priv_flag {
        let iv = build_priv_iv(priv_key, &usm.priv_params, engine_boots, engine_time, priv_proto);
        let enc = cfb_encrypt(&scoped_pdu, priv_key, &iv, priv_proto);
        encode_octet_string(&enc)
    } else {
        scoped_pdu
    };
    whole_msg.extend_from_slice(&msg_data_bytes);

    // Compute auth MAC
    let auth_mac = compute_hmac(auth_key, &whole_msg, auth_proto);

    // Rebuild with real auth params
    let usm_auth = V3SecurityParams {
        engine_id: engine_id.to_vec(),
        engine_boots,
        engine_time,
        username: username.as_bytes().to_vec(),
        auth_params: auth_mac,
        priv_params: usm.priv_params,
    };
    let usm_auth_enc = usm_auth.encode();

    let mut final_msg = Vec::new();
    final_msg.extend(encode_integer(3));
    final_msg.extend(&global_data_enc);
    final_msg.extend(encode_octet_string(&usm_auth_enc));
    final_msg.extend_from_slice(&msg_data_bytes);

    Ok(encode_sequence(&final_msg))
}

fn build_scoped_pdu(engine_id: &[u8], oid: &str) -> Result<Vec<u8>, String> {
    let oid_encoded = encode_oid(oid)?;
    let mut varbind = Vec::new();
    varbind.extend_from_slice(&oid_encoded);
    varbind.extend(encode_null());
    let wrapped_vb = encode_sequence(&varbind);
    let varbinds = encode_sequence(&wrapped_vb);

    let mut pdu_body = Vec::new();
    pdu_body.extend(encode_integer(rand::thread_rng().gen_range(0..0x7FFFFFFF))); // request-id
    pdu_body.extend(encode_integer(0)); // error
    pdu_body.extend(encode_integer(0)); // error-index
    pdu_body.extend(&varbinds);

    let pdu = tagged(0xA0, &pdu_body);

    let mut scoped = Vec::new();
    scoped.extend(encode_octet_string(engine_id)); // contextEngineID
    scoped.extend(encode_octet_string(b""));        // contextName
    scoped.extend(&pdu);

    Ok(encode_sequence(&scoped))
}

fn build_discovery_request() -> Vec<u8> {
    let msg_id: u32 = rand::thread_rng().gen_range(0..0x7FFFFFFF);
    let msg_flags: u8 = 0x04; // reportable only, noAuthNoPriv

    let mut global_data = Vec::new();
    global_data.extend(encode_integer(msg_id));
    global_data.extend(encode_integer(65535));
    global_data.extend(encode_octet_string(&[msg_flags]));
    global_data.extend(encode_integer(3)); // USM
    let global_data_enc = encode_sequence(&global_data);

    // Empty engine ID for discovery
    let usm = V3SecurityParams {
        engine_id: vec![],
        engine_boots: 0,
        engine_time: 0,
        username: vec![],
        auth_params: vec![],
        priv_params: vec![],
    };
    let usm_enc = usm.encode();

    // Empty scoped PDU (no varbinds → triggers REPORT)
    let empty_pdu_body = {
        let mut b = Vec::new();
        b.extend(encode_integer(msg_id));
        b.extend(encode_integer(0));
        b.extend(encode_integer(0));
        b.extend(encode_sequence(&[])); // empty varbind list
        b
    };
    let empty_pdu = tagged(0xA0, &empty_pdu_body);
    let mut scoped = Vec::new();
    scoped.extend(encode_octet_string(b""));
    scoped.extend(encode_octet_string(b""));
    scoped.extend(&empty_pdu);
    let scoped_enc = encode_sequence(&scoped);

    let mut whole_msg = Vec::new();
    whole_msg.extend(encode_integer(3));
    whole_msg.extend(&global_data_enc);
    whole_msg.extend(encode_octet_string(&usm_enc));
    whole_msg.extend_from_slice(&scoped_enc); // scoped is SEQUENCE, not wrapped in OCTET STRING (noAuthNoPriv)

    encode_sequence(&whole_msg)
}

fn extract_v3_params(data: &[u8]) -> Result<V3ParsedResponse, String> {
    let mut pos = 0;
    let (tag, seq_data) = decode_tlv(data, &mut pos)?;
    if tag != 0x30 { return Err(format!("期望SEQUENCE, 得到 0x{:02X}", tag)); }

    let mut sp = 0;
    // version
    let (tag, _ver) = decode_tlv(&seq_data, &mut sp)?;
    if tag != 0x02 { return Err(format!("期望INTEGER版本, 得到 0x{:02X}", tag)); }

    // msgGlobalData
    let (tag, _global) = decode_tlv(&seq_data, &mut sp)?;
    if tag != 0x30 { return Err(format!("期望HeaderData, 得到 0x{:02X}", tag)); }

    // msgSecurityParameters
    let (tag, sec_params_raw) = decode_tlv(&seq_data, &mut sp)?;
    if tag != 0x04 { return Err(format!("期望msgSecurityParameters, 得到 0x{:02X}", tag)); }

    // Parse USM
    let mut up = 0;
    let (tag, usm_data) = decode_tlv(&sec_params_raw, &mut up)?;
    if tag != 0x30 { return Err(format!("期望USM参数, 得到 0x{:02X}", tag)); }

    let mut ui = 0;
    let (_, eid) = decode_tlv(&usm_data, &mut ui)?;
    let (_, boots_data) = decode_tlv(&usm_data, &mut ui)?;
    let (_, time_data) = decode_tlv(&usm_data, &mut ui)?;
    let (_, _uname) = decode_tlv(&usm_data, &mut ui)?;

    // msgData: OCTET STRING (0x04) when encrypted, SEQUENCE (0x30) when plain
    let (tag, msg_data_val) = decode_tlv(&seq_data, &mut sp)?;
    let (scoped_pdu, is_encrypted) = match tag {
        0x04 => (msg_data_val, true),
        0x30 => {
            // Unencrypted — reconstruct full TLV for parse_scoped_pdu
            let mut full = vec![0x30];
            full.extend(encode_length(msg_data_val.len()));
            full.extend_from_slice(&msg_data_val);
            (full, false)
        }
        _ => return Err(format!("期望msgData (0x04或0x30), 得到 0x{:02X}", tag)),
    };

    Ok(V3ParsedResponse {
        engine_id: eid,
        engine_boots: decode_u32_integer(&boots_data),
        engine_time: decode_u32_integer(&time_data),
        scoped_pdu,
        is_encrypted,
    })
}

fn parse_scoped_pdu(data: &[u8], elapsed: u64, raw_hex: Option<String>) -> Result<SnmpResult, String> {
    let mut pos = 0;
    let (tag, scoped) = decode_tlv(data, &mut pos)?;
    if tag != 0x30 { return Err(format!("期望ScopedPdu, 得到 0x{:02X}", tag)); }

    let mut sp = 0;
    let (_, _ctx_eid) = decode_tlv(&scoped, &mut sp)?; // contextEngineID
    let (_, _ctx_name) = decode_tlv(&scoped, &mut sp)?; // contextName
    let (tag, pdu_data) = decode_tlv(&scoped, &mut sp)?;

    if tag == 0xA8 {
        // REPORT PDU - extract the OID
        return parse_report_pdu(&pdu_data, elapsed, raw_hex);
    }
    if tag != 0xA2 {
        return Err(format!("期望GetResponse (0xA2), 得到 0x{:02X}", tag));
    }

    parse_pdu_varbind(&pdu_data, elapsed, raw_hex)
}

fn usm_stats_oid_name(oid: &str) -> &'static str {
    match oid {
        "1.3.6.1.6.3.15.1.1.1.0" => "不支持的安全级别 (unsupportedSecurityLevel)",
        "1.3.6.1.6.3.15.1.1.2.0" => "时间窗口过期 (notInTimeWindow)",
        "1.3.6.1.6.3.15.1.1.3.0" => "未知用户名 (unknownUserName)",
        "1.3.6.1.6.3.15.1.1.4.0" => "未知引擎ID (unknownEngineID)",
        "1.3.6.1.6.3.15.1.1.5.0" => "认证失败 (wrongDigest)",
        "1.3.6.1.6.3.15.1.1.6.0" => "解密错误 (decryptionError)",
        _ => "未知USM错误",
    }
}

fn parse_report_pdu(data: &[u8], elapsed: u64, raw_hex: Option<String>) -> Result<SnmpResult, String> {
    let mut pp = 0;
    let (_, _req) = decode_tlv(data, &mut pp)?;
    let (_, _err) = decode_tlv(data, &mut pp)?;
    let (_, _idx) = decode_tlv(data, &mut pp)?;

    let (tag, vb_list) = decode_tlv(data, &mut pp)?;
    if tag != 0x30 { return Ok(SnmpResult {
        oid: String::new(), value: None, value_type: None,
        error: Some("SNMP引擎返回REPORT（无法解析详情）".into()),
        response_time_ms: elapsed, raw_hex,
    });}

    let mut vp = 0;
    let (tag, vb) = decode_tlv(&vb_list, &mut vp)?;
    if tag != 0x30 { return Ok(SnmpResult {
        oid: String::new(), value: None, value_type: None,
        error: Some("SNMP引擎返回REPORT（无法解析详情）".into()),
        response_time_ms: elapsed, raw_hex,
    });}

    let mut vi = 0;
    let (tag, oid_data) = decode_tlv(&vb, &mut vi)?;
    if tag == 0x06 {
        let report_oid = decode_oid_value(&oid_data);
        let msg = usm_stats_oid_name(&report_oid);
        return Ok(SnmpResult {
            oid: report_oid.clone(), value: None, value_type: None,
            error: Some(format!("SNMP引擎报告: {} ({})", msg, report_oid)),
            response_time_ms: elapsed, raw_hex,
        });
    }

    Ok(SnmpResult {
        oid: String::new(), value: None, value_type: None,
        error: Some("SNMP引擎返回REPORT".into()),
        response_time_ms: elapsed, raw_hex,
    })
}

fn parse_pdu_varbind(data: &[u8], elapsed: u64, raw_hex: Option<String>) -> Result<SnmpResult, String> {
    let mut pp = 0;
    let (_, req_data) = decode_tlv(data, &mut pp)?;
    let _req_id = decode_u32_integer(&req_data);
    let (_, err_data) = decode_tlv(data, &mut pp)?;
    let error_status = decode_u32_integer(&err_data);

    if error_status != 0 {
        let (_, idx_data) = decode_tlv(data, &mut pp)?;
        let error_index = decode_u32_integer(&idx_data);
        return Ok(SnmpResult {
            oid: String::new(), value: None, value_type: None,
            error: Some(format!("SNMP错误: {} (错误码: {}, 位置: {})", snmp_error_name(error_status), error_status, error_index)),
            response_time_ms: elapsed, raw_hex,
        });
    }

    let (_, _idx_data) = decode_tlv(data, &mut pp)?;
    let (tag, vb_list) = decode_tlv(data, &mut pp)?;
    if tag != 0x30 { return Err(format!("期望varbind列表, 得到 0x{:02X}", tag)); }

    let mut vp = 0;
    let (tag, vb_data) = decode_tlv(&vb_list, &mut vp)?;
    if tag != 0x30 { return Err(format!("期望varbind, 得到 0x{:02X}", tag)); }

    let mut vi = 0;
    let (tag, oid_data) = decode_tlv(&vb_data, &mut vi)?;
    if tag != 0x06 { return Err(format!("期望OID, 得到 0x{:02X}", tag)); }
    let result_oid = decode_oid_value(&oid_data);

    let (tag, val_data) = decode_tlv(&vb_data, &mut vi)?;
    let (type_name, value_str) = format_snmp_value(tag, &val_data);

    Ok(SnmpResult {
        oid: result_oid,
        value: Some(value_str), value_type: Some(type_name),
        error: None, response_time_ms: elapsed, raw_hex,
    })
}

// ============================================================================
// Public V3 API
// ============================================================================

#[allow(clippy::too_many_arguments)]
pub async fn snmp_v3_get(
    ip: &str,
    username: &str,
    auth_protocol: AuthProtocol,
    auth_password: &str,
    priv_protocol: PrivProtocol,
    priv_password: &str,
    oid: &str,
    timeout_secs: u64,
) -> Result<SnmpResult, String> {
    validate_ip(ip)?;
    // Step 1: Engine discovery
    let (engine_id, mut engine_boots, mut engine_time) = discover_engine(ip, timeout_secs).await?;

    // Step 2: Derive keys (engine ID doesn't change on retry)
    let auth_key = localize_auth_key(auth_password.as_bytes(), &engine_id, auth_protocol);
    let priv_key = if priv_protocol != PrivProtocol::None {
        localize_priv_key(priv_password.as_bytes(), &engine_id, priv_protocol, auth_protocol)
    } else {
        vec![]
    };

    // Step 3: Send GET, retry once if time window expired
    for attempt in 0..=1 {
        let request_bytes = build_v3_message(
            &engine_id, engine_boots, engine_time,
            username, &auth_key, &priv_key,
            auth_protocol, priv_protocol, oid,
        )?;

        let socket = std::net::UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| format!("无法创建UDP套接字: {}", e))?;
        socket.set_read_timeout(Some(std::time::Duration::from_secs(timeout_secs)))
            .map_err(|e| format!("无法设置超时: {}", e))?;

        let addr = format!("{}:161", ip);
        let start = std::time::Instant::now();
        socket.send_to(&request_bytes, &addr)
            .map_err(|e| format!("发送SNMP v3请求失败: {}", e))?;

        let mut buf = [0u8; 4096];
        match socket.recv_from(&mut buf) {
            Ok((len, _src)) => {
                let elapsed = start.elapsed().as_millis() as u64;
                let response = &buf[..len];
                let raw_hex = Some(hex_encode(response));

                let parsed = extract_v3_params(response)?;

                let scoped_pdu = if parsed.is_encrypted {
                    parse_response_usm_and_decrypt(response, &priv_key, priv_protocol)?
                } else {
                    parsed.scoped_pdu
                };

                let result = parse_scoped_pdu(&scoped_pdu, elapsed, raw_hex)?;

                // If time window error, re-sync boots/time from response and retry
                if attempt == 0
                    && result.error.as_ref().is_some_and(|e| is_time_window_error(e))
                {
                    engine_boots = parsed.engine_boots;
                    engine_time = parsed.engine_time;
                    tracing::info!("SNMP v3 时间窗口过期，自动重试 (boots={}, time={})", engine_boots, engine_time);
                    continue;
                }

                return Ok(result);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                return Ok(SnmpResult {
                    oid: oid.to_string(), value: None, value_type: None,
                    error: Some("SNMP v3超时".into()),
                    response_time_ms: start.elapsed().as_millis() as u64, raw_hex: None,
                });
            }
            Err(e) => {
                return Ok(SnmpResult {
                    oid: oid.to_string(), value: None, value_type: None,
                    error: Some(format!("接收SNMP v3响应失败: {}", e)),
                    response_time_ms: start.elapsed().as_millis() as u64, raw_hex: None,
                });
            }
        }
    }

    Err("SNMP v3 时间同步失败，请重试".into())
}

fn is_time_window_error(error: &str) -> bool {
    error.contains("notInTimeWindow")
}

async fn discover_engine(ip: &str, timeout_secs: u64) -> Result<(Vec<u8>, u32, u32), String> {
    let request = build_discovery_request();

    let socket = std::net::UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("引擎发现-创建UDP套接字失败: {}", e))?;
    socket.set_read_timeout(Some(std::time::Duration::from_secs(timeout_secs)))
        .map_err(|e| format!("引擎发现-设置超时失败: {}", e))?;

    let addr = format!("{}:161", ip);
    socket.send_to(&request, &addr)
        .map_err(|e| format!("引擎发现-发送请求失败: {}", e))?;

    let mut buf = [0u8; 4096];
    match socket.recv_from(&mut buf) {
        Ok((len, _src)) => {
            let parsed = extract_v3_params(&buf[..len])?;
            if parsed.engine_id.is_empty() {
                return Err("引擎发现失败: 设备返回空的引擎ID".into());
            }
            tracing::info!(
                "SNMP v3 引擎发现成功: engineID={}, boots={}, time={}",
                hex_encode(&parsed.engine_id),
                parsed.engine_boots,
                parsed.engine_time
            );
            Ok((parsed.engine_id, parsed.engine_boots, parsed.engine_time))
        }
        Err(e) => Err(format!("引擎发现失败 ({}): {}", ip, e)),
    }
}

fn parse_response_usm_and_decrypt(
    data: &[u8], priv_key: &[u8], priv_proto: PrivProtocol,
) -> Result<Vec<u8>, String> {
    // Re-extract the security params to get the salt from the response
    let mut pos = 0;
    let (tag, seq_data) = decode_tlv(data, &mut pos)?;
    if tag != 0x30 { return Err("解析失败".into()); }

    let mut sp = 0;
    let (tag, _ver) = decode_tlv(&seq_data, &mut sp)?;
    if tag != 0x02 { return Err("解析失败".into()); }

    let (tag, _global) = decode_tlv(&seq_data, &mut sp)?;
    if tag != 0x30 { return Err("解析失败".into()); }

    let (tag, sec_raw) = decode_tlv(&seq_data, &mut sp)?;
    if tag != 0x04 { return Err("解析失败".into()); }

    // Parse USM to get priv params
    let mut up = 0;
    let (tag, usm_data) = decode_tlv(&sec_raw, &mut up)?;
    if tag != 0x30 { return Err("解析失败".into()); }

    let mut ui = 0;
    let (_, _eid) = decode_tlv(&usm_data, &mut ui)?;
    let (_, _boots) = decode_tlv(&usm_data, &mut ui)?;
    let (_, _time) = decode_tlv(&usm_data, &mut ui)?;
    let (_, _uname) = decode_tlv(&usm_data, &mut ui)?;
    let (_, _auth) = decode_tlv(&usm_data, &mut ui)?;
    let (_, priv_params) = decode_tlv(&usm_data, &mut ui)?;

    // scoped PDU
    let (tag, scoped_enc) = decode_tlv(&seq_data, &mut sp)?;
    if tag != 0x04 { return Err("解析失败".into()); }

    if priv_params.is_empty() || priv_proto == PrivProtocol::None {
        return Ok(scoped_enc);
    }

    Ok(cfb_decrypt(&scoped_enc, priv_key, &priv_params, priv_proto))
}
