//! Settings saved in flash (a small key-value log, so writes wear the flash evenly).
//! The region sits just past the end of the firmware; see memory.x.

use core::ops::Range;
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_rp::Peri;
use embassy_rp::flash::{Blocking, Flash};
use embassy_rp::peripherals::FLASH;
use sequential_storage::cache::{Cache, Uncached};
use sequential_storage::map::{MapConfig, MapStorage};
use thing::Settings;

const FLASH_SIZE: usize = 2 * 1024 * 1024;
const RANGE: Range<u32> = 0x0E_0000..0x0F_0000;
const SETTINGS: u8 = 0;

type Nor = BlockingAsync<Flash<'static, FLASH, Blocking, FLASH_SIZE>>;

pub struct Store {
    map: MapStorage<u8, Nor, Cache<Uncached, Uncached, Uncached, u8>>,
    buf: [u8; 512],
}

impl Store {
    pub fn new(flash: Peri<'static, FLASH>) -> Self {
        let nor = BlockingAsync::new(Flash::new_blocking(flash));
        Self { map: MapStorage::new(nor, MapConfig::new(RANGE), Cache::new(Uncached, Uncached, Uncached)), buf: [0; 512] }
    }

    pub async fn load(&mut self) -> Option<Settings> {
        let bytes: &[u8] = self.map.fetch_item(&mut self.buf, &SETTINGS).await.ok()??;
        serde_json::from_slice(bytes).ok()
    }

    pub async fn save(&mut self, s: &Settings) -> bool {
        let Ok(bytes) = serde_json::to_vec(s) else { return false };
        self.map.store_item(&mut self.buf, &SETTINGS, &bytes.as_slice()).await.is_ok()
    }
}
