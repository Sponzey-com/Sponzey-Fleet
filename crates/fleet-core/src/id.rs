use crate::IdentityError;
use std::time::{SystemTime, UNIX_EPOCH};

const CROCKFORD_BASE32: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

pub fn generate_ulid() -> Result<String, IdentityError> {
    generate_ulid_at(SystemTime::now())
}

pub fn generate_prefixed_ulid(prefix: &str) -> Result<String, IdentityError> {
    Ok(format!("{prefix}-{}", generate_ulid()?))
}

fn generate_ulid_at(now: SystemTime) -> Result<String, IdentityError> {
    let timestamp_ms = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(0xffff_ffff_ffff) as u64;
    let mut bytes = [0_u8; 16];
    bytes[0] = (timestamp_ms >> 40) as u8;
    bytes[1] = (timestamp_ms >> 32) as u8;
    bytes[2] = (timestamp_ms >> 24) as u8;
    bytes[3] = (timestamp_ms >> 16) as u8;
    bytes[4] = (timestamp_ms >> 8) as u8;
    bytes[5] = timestamp_ms as u8;
    getrandom::getrandom(&mut bytes[6..]).map_err(|_| IdentityError::RandomFailed)?;
    Ok(encode_ulid_bytes(bytes))
}

fn encode_ulid_bytes(bytes: [u8; 16]) -> String {
    let value = u128::from_be_bytes(bytes);
    let mut encoded = [b'0'; 26];
    for index in (0..26).rev() {
        let shift = (25 - index) * 5;
        let digit = ((value >> shift) & 0x1f) as usize;
        encoded[index] = CROCKFORD_BASE32[digit];
    }
    String::from_utf8_lossy(&encoded).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn generated_ulid_uses_crockford_base32_shape() {
        let value = generate_ulid().unwrap();

        assert_eq!(value.len(), 26);
        assert!(value.bytes().all(|byte| CROCKFORD_BASE32.contains(&byte)));
    }

    #[test]
    fn generated_ulid_sorts_by_timestamp_prefix() {
        let earlier = generate_ulid_at(UNIX_EPOCH + Duration::from_millis(1)).unwrap();
        let later = generate_ulid_at(UNIX_EPOCH + Duration::from_millis(2)).unwrap();

        assert!(earlier < later);
    }

    #[test]
    fn prefixed_ulid_keeps_semantic_prefix() {
        let value = generate_prefixed_ulid("job").unwrap();

        assert!(value.starts_with("job-"));
        assert_eq!(value.len(), 30);
    }
}
