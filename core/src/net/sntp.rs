//! Wall-clock time from an NTP server (one SNTP request).

use super::{Net, NetError};

const HOST: &str = "pool.ntp.org";
const NTP_TO_UNIX: i64 = 2_208_988_800;

/// Unix seconds (UTC).
pub async fn now<N: Net>(net: &mut N) -> Result<i64, NetError> {
    let mut q = [0u8; 48];
    q[0] = 0x1B; // LI 0, version 3, client mode
    let mut r = [0u8; 48];
    let n = net.udp_exchange(HOST, 123, &q, &mut r).await?;
    if n < 44 {
        return Err(NetError::Protocol);
    }
    let secs = u32::from_be_bytes([r[40], r[41], r[42], r[43]]) as i64;
    if secs == 0 {
        return Err(NetError::Protocol);
    }
    Ok(secs - NTP_TO_UNIX)
}
