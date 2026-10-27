// Verify the real exported graph at phone and desktop sizes.
// Run after `bun run docs:build` and `bash scripts/demo.sh <binary> docs/public/demo`.
import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { readFile } from 'node:fs/promises'
import { existsSync } from 'node:fs'
import { extname, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright-core'

const root = resolve(fileURLToPath(new URL('..', import.meta.url)))
const dist = resolve(root, 'docs/.vitepress/dist')
assert(existsSync(resolve(dist, 'demo/excalidraw/index.html')), 'Build the three demo maps and docs first')

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
await new Promise((resolveListen) => server.listen(0, '127.0.0.1', resolveListen))
const origin = `http://127.0.0.1:${server.address().port}`
const browser = await chromium.launch({
  executablePath: process.env.CHROMIUM_PATH || '/usr/bin/chromium',
  args: ['--no-sandbox'],
})
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

try {
  for (const viewport of [{ name: 'mobile', width: 390, height: 844 }, { name: 'desktop', width: 1440, height: 1000 }]) {
    for (const demo of ['excalidraw', 'axum', 'flask']) {
      const context = await browser.newContext({ viewport: { width: viewport.width, height: viewport.height } })
      const page = await context.newPage()
      const errors = []
      page.on('pageerror', (error) => errors.push(String(error)))
      const embedded = demo === 'excalidraw'
      console.log(`Checking ${demo} ${viewport.name}`)
      await page.goto(`${origin}/repomap/${embedded ? 'demo/' : `demo/${demo}/`}`, { waitUntil: 'networkidle' })
      const frame = embedded ? await (await page.locator('iframe.demo-frame').elementHandle()).contentFrame() : page
      assert(frame, `${demo} iframe is missing`)
      const result = await checkControls(frame, demo, viewport.name, embedded ? page : null)
      if (embedded) {
        const input = frame.locator('#search input')
        await input.fill('restoreElements')
        await input.press('Enter')
        await frame.locator('#panel.open .fname').waitFor({ state: 'visible' })
        assert.equal(await frame.locator('#panel.open .fname').textContent(), 'restore.ts')
        await frame.getByTitle('Files that depend on this one (i)').click()
        const summary = await frame.locator('#panel .summary').textContent()
        assert(summary.includes('18 files depend on this directly') && summary.includes('328 more indirectly'), `Graph interaction regressed: ${summary}`)
        await frame.locator('#panel .close').click()
        assert(await frame.locator('#panel.open').count() === 0, 'Closing the detail panel did not restore exploration')
      }
      assert.deepEqual(errors, [], `${demo} ${viewport.name} browser errors`)
      results.push(result)
      await context.close()
    }
  }
  console.log(`Responsive graph PASS: ${results.length} demo/viewports; embedded excalidraw search, impact and panel close PASS`)
} finally {
  await browser.close()
  await new Promise((resolveClose) => server.close(resolveClose))
}
