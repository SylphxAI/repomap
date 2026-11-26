import assert from 'node:assert/strict'

const videoSelector = '.sx-media video'
const mp4 = '/repomap/img/demo.mp4'
const posterPath = '/repomap/img/hero-excalidraw.webp'
const phone = { width: 390, height: 844 }
const desktop = { width: 1440, height: 1000 }

async function waitForPlayback(page) {
  await page.waitForFunction((selector) => {
    const video = document.querySelector(selector)
    return !video.paused && video.currentTime > 0 && video.getVideoPlaybackQuality().totalVideoFrames > 1
  }, videoSelector)
}

export async function checkHomeMedia(browser, home, recordErrors) {
  for (const [name, viewport, preference] of [
    ['mobile', phone], ['desktop', desktop], ['reduced motion', phone, 'motion'], ['Save-Data', phone, 'data'],
  ]) {
    const context = await browser.newContext({ locale: 'en-US', viewport, reducedMotion: preference === 'motion' ? 'reduce' : 'no-preference' })
    try {
      if (preference === 'data') {
        await context.addInitScript(() => Object.defineProperty(navigator.connection, 'saveData', { value: true }))
      }
      const page = await context.newPage()
      const errors = recordErrors(page)
      const requests = []
      page.on('request', (request) => {
        if (new URL(request.url()).pathname === mp4) requests.push(request)
      })
      const posterResponse = page.waitForResponse((response) => new URL(response.url()).pathname === posterPath)
      await page.goto(home, { waitUntil: 'networkidle' })
      assert((await posterResponse).ok(), `${name}: real poster was served`)
      await page.evaluate(() => document.fonts.ready.then(() => {}))
      const video = page.locator(videoSelector)
      const box = await video.boundingBox()
      assert.equal(await video.getAttribute('poster'), posterPath)
      assert.equal(await video.getAttribute('width'), '1600')
      assert.equal(await video.getAttribute('height'), '1000')
      assert.match(await video.getAttribute('aria-label'), /^repomap demo: the map of excalidraw/)
      assert(Math.abs(box.height - (box.width - 2) * 1000 / 1600 - 2) < 1, `${name}: reserved poster ratio`)
      for (const link of ['Live demo', 'Try it live →']) {
        const links = await page.getByRole('link', { name: link, exact: true }).evaluateAll((elements) => elements.map((el) => el.getAttribute('href')))
        assert(links.length && links.every((href) => href === '/repomap/demo'), link)
      }

      if (viewport === phone) {
        assert(box.y >= viewport.height, `${name}: video starts offscreen`)
        assert.equal(requests.length, 0, `${name}: no MP4 request before intersection`)
        assert.equal(await video.getAttribute('src'), null)
        await video.scrollIntoViewIfNeeded()
      } else {
        assert(box.y >= 0 && box.y < viewport.height, 'desktop: video starts in viewport')
      }

      if (preference) {
        await page.waitForFunction((selector) => {
          const video = document.querySelector(selector)
          return video.controls && video.networkState === HTMLMediaElement.NETWORK_IDLE
        }, videoSelector)
        assert.deepEqual(await video.evaluate((el) => [el.autoplay, el.preload, el.paused, el.currentTime]), [false, 'none', true, 0])
        assert.equal(requests.length, 0, `${name}: no MP4 request while visible before Play`)
        await video.press('Space') // First keyboard activation of native playback (no JS play() call).
      }

      await waitForPlayback(page)
      assert(requests.length > 0, `${name}: real MP4 requested for playback`)
      const after = await video.boundingBox()
      assert.equal(after.width, box.width, `${name}: width unchanged when playing`)
      assert.equal(after.height, box.height, `${name}: height unchanged when playing`)
      assert.deepEqual(errors, [], `${name}: browser errors`)
    } finally {
      await context.close()
    }
  }
  console.log('Home media PASS: mobile deferral, visible desktop autoplay, first native Play in both reduced preferences, poster geometry and Live demo links')
}
