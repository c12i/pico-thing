//! Xiaomi miIO (AES-encrypted UDP 54321), for Mi-ecosystem Yeelights such as the
//! bslamp2 that the Yeelight app won't open up for LAN Control. The commands are the
//! same JSON methods as Yeelight LAN; only the transport differs. Each call does a
//! fresh handshake (which gives the device id and its clock) and then one request.

use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use alloc::format;
use alloc::vec::Vec;
use md5::{Digest, Md5};
use serde_json::Value;

use super::{Net, NetError};

pub const PORT: u16 = 54321;
const HELLO: [u8; 32] = {
    let mut h = [0xFF; 32];
    h[0] = 0x21;
    h[1] = 0x31;
    h[2] = 0x00;
    h[3] = 0x20;
    h
};

type Enc = cbc::Encryptor<aes::Aes128>;
type Dec = cbc::Decryptor<aes::Aes128>;

fn md5(parts: &[&[u8]]) -> [u8; 16] {
    let mut h = Md5::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

/// "0123...ef" (32 hex chars) -> 16 bytes.
pub fn parse_token(hex: &str) -> Option<[u8; 16]> {
    let b = hex.as_bytes();
    if b.len() != 32 {
        return None;
    }
    let nib = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = [0u8; 16];
    for (i, o) in out.iter_mut().enumerate() {
        *o = nib(b[2 * i])? << 4 | nib(b[2 * i + 1])?;
    }
    Some(out)
}

pub async fn call<N: Net>(net: &mut N, ip: &str, token: &[u8; 16], method: &str, params: &str) -> Result<Value, NetError> {
    let key = md5(&[token]);
    let iv = md5(&[&key, token]);

    let mut reply = [0u8; 1024];
    let n = net.udp_exchange(ip, PORT, &HELLO, &mut reply).await?;
    if n < 32 {
        return Err(NetError::Protocol);
    }
    let did = [reply[8], reply[9], reply[10], reply[11]];
    let stamp = u32::from_be_bytes([reply[12], reply[13], reply[14], reply[15]]);

    // Devices drop repeated ids, so vary them with the device clock
    let id = stamp % 100_000 + 1;
    let payload = format!("{{\"id\":{id},\"method\":\"{method}\",\"params\":[{params}]}}");
    let enc = Enc::new(&key.into(), &iv.into()).encrypt_padded_vec_mut::<Pkcs7>(payload.as_bytes());
    let mut packet: Vec<u8> = Vec::with_capacity(32 + enc.len());
    packet.extend_from_slice(&0x2131u16.to_be_bytes());
    packet.extend_from_slice(&((32 + enc.len()) as u16).to_be_bytes());
    packet.extend_from_slice(&[0, 0, 0, 0]);
    packet.extend_from_slice(&did);
    packet.extend_from_slice(&(stamp + 1).to_be_bytes());
    let sum = md5(&[&packet, token, &enc]);
    packet.extend_from_slice(&sum);
    packet.extend_from_slice(&enc);

    let n = net.udp_exchange(ip, PORT, &packet, &mut reply).await?;
    if n <= 32 {
        return Err(NetError::Protocol);
    }
    let dec = Dec::new(&key.into(), &iv.into()).decrypt_padded_vec_mut::<Pkcs7>(&reply[32..n]).map_err(|_| NetError::Protocol)?;
    let end = dec.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    let v: Value = serde_json::from_slice(&dec[..end]).map_err(|_| NetError::Protocol)?;
    if v.get("error").is_some() {
        return Err(NetError::Protocol);
    }
    Ok(v.get("result").cloned().unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens() {
        assert_eq!(parse_token("000102030405060708090a0b0c0d0eff").unwrap()[15], 0xff);
        assert!(parse_token("xyz").is_none());
    }
}
