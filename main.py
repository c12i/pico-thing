import lcd
import gfx

display = lcd.LCD()
# Allocate the 77KB framebuffer first, before other imports fragment the heap
cv = gfx.Canvas(display)

import asyncio
import companion
import net
import pomodoro
import settings
import touch
import ui
import weather

prefs = settings.load()
cv.set_palette(gfx.THEMES.get(prefs.get('theme'), gfx.THEME))
display.brightness(prefs.get('brightness', 100))
app = ui.App(cv, touch.Touch(display.spi, display.baud, prefs['calib']), weather.Weather(), prefs)
app.pomo = pomodoro.Pomodoro(prefs)


def refresh_weather():
    def done(ok, _err):
        hook = getattr(app.screen, 'on_weather', None)
        if ok and hook:
            hook()
    app.background(app.weather.update(), done)


def sync_time():
    def done(ok, _err):
        if ok:
            app.time_ok = True
    app.background(net.sync_time(), done)


def poll_companion():
    if companion.BASE and net.connected():
        app.background(companion.status(), on_companion)


def on_companion(st, err):
    if err or not st:
        return
    app.companion = st
    screen = app.screen
    if st.get('attention') and getattr(screen, 'is_face', False):
        import command_screen
        app.go(command_screen.AttentionScreen(app, st['attention']))
        return
    hook = getattr(screen, 'on_companion', None)
    if hook:
        hook()


async def flash_backlight(times=4):
    level = display.level
    for _ in range(times):
        display.brightness(0)
        await asyncio.sleep_ms(180)
        display.brightness(level)
        await asyncio.sleep_ms(180)


def check_pomodoro():
    p = app.pomo
    if not p.tick(app.localtime()[2]):
        return
    msg = 'Focus done - take a break' if p.finished == 'focus' else 'Break over - back to it'
    asyncio.create_task(flash_backlight())
    if companion.BASE:
        app.background(companion.call('POST', '/notify?text=' + msg.replace(' ', '+')))
    if not app.busy and not hasattr(app.screen, 'tap_raw'):     # never interrupt calibration
        import pomodoro_screen
        app.go(pomodoro_screen.PomodoroScreen(app, alert=True))


def keep_online():
    if not net.connected():
        app.background(net.connect(5))


async def boot():
    app.status('Connecting to Wi-Fi')
    if await net.connect():
        app.status('Syncing time')
        app.time_ok = await net.sync_time()
        app.status('Fetching weather')
        await app.weather.update()
    else:
        app.status('Offline - check config.py')
        await asyncio.sleep(2)

    app.every(15 * 60, refresh_weather)
    app.every(6 * 3600, sync_time)
    app.every(60, keep_online)
    app.every(30, poll_companion)
    app.every(1, check_pomodoro)
    if companion.BASE:
        await companion.serve(poll_companion)   # the Mac pokes us on changes

    # Holding a finger on the screen while booting forces recalibration
    if app.touch.irq():
        app.home()
    else:
        import screens
        app.go(screens.Calibrate(app))
    poll_companion()
    await app.run()


asyncio.run(boot())
