// Verify generated Pages graph and text modes on phone and desktop. Generate the
// demos with `scripts/demo.sh <binary> docs/public/demo`, then `bun run docs:build`.
// REPOMAP_DEMO_URL can point the two-mode probe at the deployed Excalidraw report.
import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { readFile } from 'node:fs/promises'
import { existsSync } from 'node:fs'
import { extname, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright-core'
import { checkHomeMedia } from './home-media.browser-test.mjs'

const root = resolve(fileURLToPath(new URL('..', import.meta.url)))
const dist = resolve(root, 'docs/.vitepress/dist')
const liveURL = process.env.REPOMAP_DEMO_URL
if (!liveURL) assert(existsSync(resolve(dist, 'demo/excalidraw/index.html')), 'Build the three demo maps and docs first')

const types = {
  '.html': 'text/html; charset=utf-8', '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8', '.json': 'application/json',
  '.svg': 'image/svg+xml', '.png': 'image/png', '.webp': 'image/webp',
  '.gif': 'image/gif', '.mp4': 'video/mp4', '.wasm': 'application/wasm',
}
const server = createServer(async (req, res) => {
  try {
    const pathname = decodeURIComponent(new URL(req.url, 'http://local').pathname)
    let relative = pathname.replace(/^\/repomap\/?/, '')
    const candidates = relative.endsWith('/') ? [relative + 'index.html', relative.slice(0, -1) + '.html'] : [relative || 'index.html', relative + '.html']
    let file
    let bytes
    for (const candidate of candidates) {
      file = resolve(dist, candidate)
      if (file !== dist && !file.startsWith(dist + sep)) {
        res.writeHead(403).end('refused')
        return
      }
      try {
        bytes = await readFile(file)
        break
      } catch (error) {
        if (error.code !== 'ENOENT') throw error
      }
    }
    if (!bytes) {
      res.writeHead(404).end('not found')
      return
    }
    res.writeHead(200, { 'content-type': types[extname(file)] || 'application/octet-stream' })
    res.end(bytes)
  } catch (error) {
    res.writeHead(error.code === 'ENOENT' ? 404 : 500).end('not found')
  }
})
if (!liveURL) await new Promise((resolveListen) => server.listen(0, '127.0.0.1', resolveListen))
const origin = liveURL ? null : `http://127.0.0.1:${server.address().port}`
const demoURL = liveURL || `${origin}/repomap/demo/excalidraw/`
const restorePath = 'packages/excalidraw/data/restore.ts'
const restoreSource = 'https://github.com/excalidraw/excalidraw/blob/817d8c553c3389650f8b4503984a6d4a5d2f0c11/' + restorePath

async function checkOverviewSearch(page, overview) {
  const query = overview.getByRole('searchbox', { name: 'Find a file or symbol' })
  const results = overview.getByRole('list', { name: 'Search results' })
  const modules = overview.locator('details')
  assert.equal(await results.isVisible(), false, 'Do not render matches for an empty query')
  assert.equal(await modules.locator('ul').count(), 0, 'Keep unopened modules lazy')
  await page.keyboard.press('Tab')
  assert(await query.evaluate((el) => document.activeElement === el), 'Search must be the first keyboard stop')
  await page.keyboard.type('restoreElements')
  const hit = results.getByRole('link', { name: 'restoreElements in ' + restorePath, exact: true })
  assert.equal(await hit.getAttribute('href'), restoreSource + '#L490')
  assert.equal(await overview.getByRole('status').textContent(), 'Showing 1 match.')
  await page.keyboard.press('Tab')
  assert(await hit.evaluate((el) => document.activeElement === el), 'Tab must reach the real source link directly')
  assert(await hit.evaluate((el) => {
    const box = el.getBoundingClientRect()
    return box.left >= 0 && box.right <= innerWidth && box.top >= 0 && box.bottom <= innerHeight
  }), 'Focused source link must fit the viewport')
  assert.equal(await overview.locator('details[open]').count(), 0, 'Search must not expand any module')

  await page.keyboard.press('Shift+Tab')
  assert(await query.evaluate((el) => document.activeElement === el), 'Shift+Tab must return to search')
  await page.keyboard.press('ControlOrMeta+a')
  await page.keyboard.type('RESTORE.TS')
  assert.equal(await results.getByRole('link', { name: restorePath, exact: true }).getAttribute('href'), restoreSource)
  await page.keyboard.press('ControlOrMeta+a')
  await page.keyboard.type('no_such_repomap_test_symbol_123')
  assert.equal(await overview.getByRole('status').textContent(), 'No matches.')
  assert.equal(await results.isVisible(), false, 'No matches must clear previous links')
  await page.keyboard.press('ControlOrMeta+a')
  await page.keyboard.press('Backspace')
  assert.equal(await overview.getByRole('status').textContent(), '')
  assert.equal(await results.isVisible(), false)
}

async function checkOverviewDeepLink(page, overview) {
  // `/` focuses search from anywhere outside a field, as in graph mode.
  await overview.locator('h1').click()
  await page.keyboard.press('/')
  assert(await overview.getByRole('searchbox', { name: 'Find a file or symbol' }).evaluate((el) => document.activeElement === el), '/ must focus search')
  // A #path link shows that file with the graph panel's counts, on hash change and on load.
  await page.evaluate((path) => { location.hash = encodeURIComponent(path) }, restorePath)
  const linked = overview.getByRole('region', { name: restorePath })
  await linked.waitFor({ state: 'visible' })
  assert(await linked.getByRole('heading', { name: restorePath }).evaluate((el) => document.activeElement === el), 'Linked file must take focus')
  await page.reload()
  await linked.waitFor({ state: 'visible' })
  assert.equal(await linked.locator('.overview-stats').textContent(), '16 symbols · used by 18 files · excalidraw/element')
  assert.equal(await linked.getByRole('link', { name: 'Open source', exact: true }).getAttribute('href'), restoreSource)
  assert.equal(await linked.getByRole('link', { name: 'restoreElements', exact: true }).count(), 1)
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Linked file overflows viewport horizontally')
}

const swiftShader = ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader']
const chromiumPath = process.env.CHROMIUM_PATH || '/usr/bin/chromium'

function recordErrors(page) {
  const errors = []
  page.on('console', (message) => { if (message.type() === 'error') errors.push(message.text()) })
  page.on('pageerror', (error) => errors.push(String(error)))
  return errors
}

async function checkExcalidrawInteraction(frame) {
  const input = frame.locator('#search input')
  await input.fill('restoreElements')
  await input.press('Enter')
  const path = frame.locator('#panel.open .fpath')
  await path.waitFor({ state: 'visible' })
  assert.equal(await path.textContent(), restorePath)
  await frame.getByTitle('Files that depend on this one (i)').click()
  const summary = await frame.locator('#panel .summary').textContent()
  assert.match(summary, /18 files depend on this directly, 328 more indirectly/)
  await frame.locator('#panel .close').click()
  assert.equal(await frame.locator('#panel.open').count(), 0)
}

async function checkDemo(args, mode, viewports, setup) {
  const browser = await chromium.launch({ executablePath: chromiumPath, args })
  try {
    for (const [width, height] of viewports) {
      const page = await browser.newPage({ locale: 'en-US', viewport: { width, height } })
      page.setDefaultTimeout(10_000)
      const errors = recordErrors(page)
      if (setup) await page.addInitScript(setup)
      const response = await page.goto(demoURL)
      assert.equal(response?.status(), 200)
      const overview = page.locator('#overview')
      if (mode === 'graph') {
        await page.locator('#loading.done').waitFor({ state: 'attached' })
        const canvas = page.locator('#stage .sigma-nodes')
        await canvas.waitFor({ state: 'visible' })
        const gpu = await canvas.evaluate((el) => {
          const gl = el.getContext('webgl2')
          const info = gl?.getExtension('WEBGL_debug_renderer_info')
          return gl && info && gl.getParameter(info.UNMASKED_RENDERER_WEBGL)
        })
        assert.match(String(gpu), /SwiftShader/i, 'Expected a real software WebGL context on the graph canvas')
        assert.equal(await overview.count(), 0)
        await checkExcalidrawInteraction(page)
        console.log(`graph ${width}x${height}: ${gpu}; restore.ts search and impact PASS`)
      } else {
        await overview.getByRole('heading', { name: 'excalidraw: architecture overview' }).waitFor()
        assert.match(await overview.locator('.overview-stats').textContent(), /540 files · 3,592 symbols/)
        assert.match(await overview.innerText(), /The interactive graph needs WebGL/)
        assert.equal(await page.locator('#stage canvas').count(), 0)
        assert.equal(await page.locator('#controls').isVisible(), false)
        assert.equal(await page.locator('#loading').isVisible(), false)
        if (mode === 'disabled') {
          const webgl = await page.evaluate(() => ['webgl2', 'webgl', 'experimental-webgl'].some((kind) => document.createElement('canvas').getContext(kind)))
          assert.equal(webgl, false, 'The browser must actually reject WebGL contexts')
        }
        await checkOverviewSearch(page, overview)
        const module = overview.locator('summary').filter({ hasText: /^excalidraw\/element 45 files$/ })
        await module.focus()
        await module.press('Enter')
        const source = overview.getByRole('link', { name: restorePath, exact: true })
        await source.waitFor({ state: 'visible' })
        assert.equal(await source.getAttribute('href'), restoreSource)
        assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Overview overflows viewport horizontally')
        await source.scrollIntoViewIfNeeded()
        assert(await source.evaluate((el) => el.getBoundingClientRect().bottom <= innerHeight), 'Source link is clipped vertically')
        await checkOverviewDeepLink(page, overview)
        console.log(`${mode} ${width}x${height}: keyboard symbol/path search, no matches, clear, module browsing, / and #path link PASS`)
      }
      assert.deepEqual(errors, [], `${mode} ${width}x${height} console/page errors`)
      await page.close()
    }
  } finally {
    await browser.close()
  }
}

async function checkDemoModes() {
  const viewports = [[390, 844], [1440, 900]]
  await checkDemo(swiftShader, 'graph', viewports)
  await checkDemo(['--disable-webgl'], 'disabled', viewports)
  if (!process.env.REPOMAP_DEMO_URL) {
    await checkDemo(swiftShader, 'second layer refused', [[1440, 1000]], () => {
      const getContext = HTMLCanvasElement.prototype.getContext
      HTMLCanvasElement.prototype.getContext = function (type, ...args) {
        if (this.classList.contains('sigma-nodes') && /webgl/.test(type)) return null
        return getContext.call(this, type, ...args)
      }
    })
    await checkDemo(swiftShader, 'framebuffer refused', [[1440, 1000]], () => {
      WebGL2RenderingContext.prototype.createFramebuffer = () => null
    })
  }
}

const results = []

async function checkControls(frame, demo, viewportName, outerPage) {
  await frame.locator('#controls').waitFor({ state: 'visible' })
  await frame.waitForFunction(() => document.querySelector('#loading')?.classList.contains('done'))
  const dimensions = await frame.evaluate(() => ({ width: innerWidth, height: innerHeight }))
  const outer = outerPage ? await outerPage.locator('iframe.demo-frame').evaluate((el) => {
    el.scrollIntoView({ block: 'center', inline: 'nearest' })
    const r = el.getBoundingClientRect()
    return { scrollY, height: innerHeight, iframe: { top: r.top, bottom: r.bottom, height: r.height } }
  }) : null
  const buttons = []
  for (const control of ['layout', 'fit', 'edges', 'tests', 'labels']) {
    const button = frame.locator(`#controls button[data-c="${control}"]`)
    await button.waitFor({ state: 'visible' })
    const rect = await button.evaluate((el) => {
      const { x, y, width, height, right, bottom } = el.getBoundingClientRect()
      return { x, y, width, height, right, bottom }
    })
    assert(rect.x >= 0 && rect.y >= 0, `${demo} ${viewportName} ${control} starts outside the viewport`)
    assert(rect.right <= dimensions.width + 0.5, `${demo} ${viewportName} ${control} is clipped horizontally: ${JSON.stringify({ rect, dimensions })}`)
    assert(rect.bottom <= dimensions.height + 0.5, `${demo} ${viewportName} ${control} is clipped vertically`)
    if (outer) {
      assert(outer.iframe.top + rect.y >= 0 && outer.iframe.top + rect.bottom <= outer.height, `${demo} ${viewportName} ${control} is outside the outer viewport: ${JSON.stringify({ rect, outer })}`)
    }
    assert(rect.width >= 34 && rect.height >= 34, `${demo} ${viewportName} ${control} target is too small`)
    try {
      await button.click()
    } catch (error) {
      throw new Error(`${demo} ${viewportName} ${control} is not clickable: ${JSON.stringify({ rect, dimensions, outer })}\n${error}`)
    }
    buttons.push({ control, rect })
  }
  return { demo, viewport: viewportName, dimensions, buttons }
}

async function checkResponsive() {
  const browser = await chromium.launch({ executablePath: chromiumPath, args: swiftShader })
  try {
    for (const viewport of [{ name: 'mobile', width: 390, height: 844 }, { name: 'desktop', width: 1440, height: 1000 }]) {
      for (const demo of ['excalidraw', 'axum', 'flask']) {
        const context = await browser.newContext({ locale: 'en-US', viewport: { width: viewport.width, height: viewport.height } })
        const page = await context.newPage()
        const errors = recordErrors(page)
        const embedded = demo === 'excalidraw'
        console.log(`Checking ${demo} ${viewport.name}`)
        await page.goto(`${origin}/repomap/${embedded ? 'demo/' : `demo/${demo}/`}`, { waitUntil: 'networkidle' })
        const frame = embedded ? await (await page.locator('iframe.demo-frame').elementHandle()).contentFrame() : page
        assert(frame, `${demo} iframe is missing`)
        const result = await checkControls(frame, demo, viewport.name, embedded ? page : null)
        if (embedded) await checkExcalidrawInteraction(frame)
        assert.deepEqual(errors, [], `${demo} ${viewport.name} browser errors`)
        results.push(result)
        await context.close()
      }
    }
    const localized = await browser.newContext({ locale: 'ar-EG' })
    const page = await localized.newPage()
    await page.goto(`${origin}/repomap/demo/excalidraw/`)
    await page.waitForFunction(() => document.querySelector('#loading')?.classList.contains('done'))
    const stats = await page.locator('#brand .stats').textContent()
    assert.match(stats, /[٠-٩]+٬[٠-٩]{3} symbols/)
    assert.match(stats, /[٠-٩]+ modules$/)
    const module = page.locator('#legend .mod[data-c="0"]')
    assert.match(await module.locator('.count').textContent(), /^[٠-٩]+$/)
    await module.click()
    assert(await module.evaluate((el) => el.classList.contains('on')), 'Localized count must not change the module ID')
    await localized.close()
    console.log(`Responsive graph PASS: ${results.length} demo/viewports; embedded excalidraw search, impact and panel close PASS; Arabic header and legend counts PASS`)
    await checkHomeMedia(browser, `${origin}/repomap/`, recordErrors)
  } finally {
    await browser.close()
  }
}

try {
  await checkDemoModes()
  if (!liveURL) await checkResponsive()
} finally {
  if (server.listening) await new Promise((resolveClose) => server.close(resolveClose))
}
