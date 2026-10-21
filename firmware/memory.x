MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
    /* 0x0E0000..0x0F0000 holds saved settings (storage.rs). */
    FLASH : ORIGIN = 0x10000100, LENGTH = 896K - 0x100
    RAM   : ORIGIN = 0x20000000, LENGTH = 256K
}
