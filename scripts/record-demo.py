# Record the README demo: python scripts/record-demo.py http://localhost:7878/ /tmp/rec [query]
# (run `repomap serve --no-open` on excalidraw first; convert with ffmpeg, see docs/guide/ui.md)
import sys, time, os, glob, shutil
from playwright.sync_api import sync_playwright
url, outdir = sys.argv[1], sys.argv[2]
query = sys.argv[3] if len(sys.argv) > 3 else "restoreElements"
W, H = 1280, 800
CURSOR = """
window.addEventListener('DOMContentLoaded', () => {
  const c = document.createElement('div');
  c.style.cssText = 'position:fixed;z-index:99999;width:18px;height:18px;border-radius:50%;background:rgba(255,255,255,.9);box-shadow:0 0 0 4px rgba(138,164,255,.35),0 2px 8px rgba(0,0,0,.6);pointer-events:none;transform:translate(-50%,-50%);left:-50px;top:-50px;transition:transform .12s';
  document.body.appendChild(c);
  document.addEventListener('mousemove', e => { c.style.left = e.clientX + 'px'; c.style.top = e.clientY + 'px'; }, true);
  document.addEventListener('mousedown', () => c.style.transform = 'translate(-50%,-50%) scale(.7)', true);
  document.addEventListener('mouseup', () => c.style.transform = 'translate(-50%,-50%) scale(1)', true);
});
"""
shutil.rmtree(outdir, ignore_errors=True)
with sync_playwright() as p:
    b = p.chromium.launch(args=["--use-gl=angle","--use-angle=swiftshader","--enable-unsafe-swiftshader","--ignore-gpu-blocklist"])
    ctx = b.new_context(viewport={"width": W, "height": H}, record_video_dir=outdir, record_video_size={"width": W, "height": H})
    ctx.add_init_script(CURSOR)
    pg = ctx.new_page()
    def glide(x, y, steps=25):
        pg.mouse.move(x, y, steps=steps)
    pg.goto(url)
    pg.mouse.move(W/2, H/2)
    pg.wait_for_function("document.body.dataset.ready === '1'", timeout=60000)
    time.sleep(0.8)
    # hover around the map
    glide(700, 380); time.sleep(0.6); glide(820, 520); time.sleep(0.6)
    # search
    box = pg.locator("#search input").bounding_box()
    glide(box["x"] + 120, box["y"] + box["height"]/2)
    pg.mouse.click(box["x"] + 120, box["y"] + box["height"]/2)
    for ch in query:
        pg.keyboard.type(ch); time.sleep(0.07)
    time.sleep(1.2)
    first = pg.locator("#results .res").first.bounding_box()
    glide(first["x"] + 80, first["y"] + first["height"]/2, 15)
    time.sleep(0.4)
    pg.mouse.down(); pg.mouse.up()
    pg.locator("#results .res").first.dispatch_event("mousedown")
    time.sleep(2.4)
    # scroll the code a little
    code = pg.locator("pre.code")
    if code.count():
        cb = code.bounding_box()
        glide(cb["x"] + 150, cb["y"] + 100)
        pg.mouse.wheel(0, 180); time.sleep(1.0)
    # impact
    btn = pg.locator('[data-act="impact"]').bounding_box()
    glide(btn["x"] + btn["width"]/2, btn["y"] + btn["height"]/2)
    time.sleep(0.3)
    pg.mouse.click(btn["x"] + btn["width"]/2, btn["y"] + btn["height"]/2)
    time.sleep(3.2)
    glide(760, 430); time.sleep(1.2)
    pg.close(); ctx.close(); b.close()
print(glob.glob(outdir + "/*.webm")[0])
