# Driver for the Waveshare Pico-ResTouch-LCD-3.5 (ILI9488, 480x320, SPI).
# The board converts SPI to the panel's 16-bit bus, so we can send RGB565.
from machine import Pin, PWM, SPI
import machine
import micropython
import time

LCD_DC = 8
LCD_CS = 9
LCD_SCK = 10
LCD_MOSI = 11
LCD_MISO = 12
LCD_BL = 13
LCD_RST = 15
TP_CS = 16

WIDTH = 480
HEIGHT = 320
LCD_HZ = 31_250_000   # 62.5MHz is too fast for the board's shift registers

_CLK_PERI_CTRL = 0x40008048
_ASSUMED_PERI_HZ = 48_000_000


def _peri_from_sys_clock():
    # MicroPython feeds clk_peri from the 48MHz USB PLL, capping SPI at 24MHz.
    # Switch it to clk_sys (125MHz) so the panel can run at 31.25MHz. MicroPython
    # still believes clk_peri is 48MHz, so baud rates must go through spi_baud().
    machine.mem32[_CLK_PERI_CTRL] = 0          # disable, source = clk_sys
    machine.mem32[_CLK_PERI_CTRL] = 1 << 11    # enable


def spi_baud(hz):
    """Value to pass to machine.SPI to get a real clock of at most `hz`."""
    return hz * _ASSUMED_PERI_HZ // machine.freq()


def rgb(r, g, b):
    """24-bit colour -> RGB565."""
    return ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3)


@micropython.viper
def _gs4_to_565(src: ptr8, soff: int, n: int, lut: ptr32, dst: ptr32, doff: int):
    # Each source byte holds two 4-bit pixels; lut maps it straight to their
    # four big-endian RGB565 output bytes
    j = doff >> 2
    i = soff
    end = soff + n
    while i < end:
        dst[j] = lut[src[i]]
        j += 1
        i += 1


class LCD:
    def __init__(self):
        self.cs = Pin(LCD_CS, Pin.OUT, value=1)
        self.dc = Pin(LCD_DC, Pin.OUT, value=1)
        self.rst = Pin(LCD_RST, Pin.OUT, value=1)
        # Keep the touch controller deselected so it doesn't fight on the bus
        self.tp_cs = Pin(TP_CS, Pin.OUT, value=1)
        self.bl = PWM(Pin(LCD_BL), freq=2000, duty_u16=65535)
        self.level = 100
        _peri_from_sys_clock()
        self.baud = spi_baud(LCD_HZ)
        self.spi = SPI(1, self.baud, sck=Pin(LCD_SCK), mosi=Pin(LCD_MOSI), miso=Pin(LCD_MISO))
        self._line = bytearray(WIDTH * 2 * 8)
        self._init_display()

    def brightness(self, pct):
        """Backlight 0-100%. The PWM block runs off clk_sys, so the clk_peri change
        above doesn't affect it."""
        self.level = min(max(int(pct), 0), 100)
        self.bl.duty_u16(self.level * 65535 // 100)

    def _write16(self, dc, value):
        # The board's shift registers latch 16 bits per CS frame, so commands and
        # parameters each go out as their own 16-bit word (high byte ignored).
        self.dc(dc)
        self.cs(0)
        self.spi.write(bytes([0x00, value]))
        self.cs(1)

    def _cmd(self, c, data=None):
        self._write16(0, c)
        for d in data or ():
            self._write16(1, d)

    def _init_display(self):
        self.rst(1); time.sleep_ms(5)
        self.rst(0); time.sleep_ms(10)
        self.rst(1); time.sleep_ms(5)

        self._cmd(0x21)                          # display inversion on (panel needs it)
        self._cmd(0xC2, [0x33])                  # power control 3
        self._cmd(0xC5, [0x00, 0x1E, 0x80])      # VCOM
        self._cmd(0xB1, [0xB0])                  # frame rate
        self._cmd(0xE0, [0x00, 0x13, 0x18, 0x04, 0x0F, 0x06, 0x3A, 0x56,
                         0x4D, 0x03, 0x0A, 0x06, 0x30, 0x3E, 0x0F])  # +gamma
        self._cmd(0xE1, [0x00, 0x13, 0x18, 0x01, 0x11, 0x06, 0x38, 0x34,
                         0x4D, 0x06, 0x0D, 0x0B, 0x31, 0x37, 0x0F])  # -gamma
        self._cmd(0x3A, [0x55])                  # 16 bits per pixel
        self._cmd(0x11)                          # sleep out
        time.sleep_ms(120)
        self._cmd(0x29)                          # display on
        self._cmd(0xB6, [0x00, 0x62])            # display function control
        self._cmd(0x36, [0x28])                  # landscape, BGR

    def _window(self, x, y, w, h):
        x1, y1 = x + w - 1, y + h - 1
        self._cmd(0x2A, [x >> 8, x & 0xFF, x1 >> 8, x1 & 0xFF])
        self._cmd(0x2B, [y >> 8, y & 0xFF, y1 >> 8, y1 & 0xFF])
        self._cmd(0x2C)

    def fill_rect(self, x, y, w, h, color):
        self._window(x, y, w, h)
        # Panel wants big-endian pixels; send one row at a time
        row = bytes([color >> 8, color & 0xFF]) * w
        self.dc(1)
        self.cs(0)
        for _ in range(h):
            self.spi.write(row)
        self.cs(1)

    def blit_gs4(self, buf, lut, x, y, w, h):
        """Push a region of a full-screen GS4_HMSB framebuffer. lut is 256 x 4 bytes:
        for each source byte, its two pixels as big-endian RGB565."""
        x1 = min(x + w, WIDTH)
        y1 = min(y + h, HEIGHT)
        x = max(x, 0) & ~1        # 2 pixels per byte, so keep x and w even
        y = max(y, 0)
        w = (x1 - x + 1) & ~1
        if w <= 0 or y1 <= y:
            return
        self._window(x, y, w, y1 - y)
        line = self._line
        mv = memoryview(line)
        stride = w * 2
        rows = len(line) // stride
        self.dc(1)
        self.cs(0)
        r = y
        while r < y1:
            n = min(rows, y1 - r)
            for i in range(n):
                _gs4_to_565(buf, ((r + i) * WIDTH + x) >> 1, w >> 1, lut, line, i * stride)
            self.spi.write(mv[:n * stride])
            r += n
        self.cs(1)
