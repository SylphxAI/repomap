import { defineConfig } from 'vitepress'
import tokens from '../../brand/tokens.json'

const base = '/repomap/'
const url = 'https://sylphxai.github.io/repomap/'
const desc = 'A map of your codebase for AI agents: code graph, search, call paths and change impact, with an interactive graph UI. Local, no API key, MIT.'

export default defineConfig({
  base,
  title: 'repomap',
  description: desc,
  appearance: 'force-dark',
  cleanUrls: true,
  srcExclude: ['storefront/**'],
  lastUpdated: true,
  sitemap: { hostname: url },
  markdown: { theme: { light: 'github-light', dark: 'github-dark-default' } },
  head: [
    ['link', { rel: 'icon', href: `${base}favicon.ico`, sizes: '16x16 32x32 48x48' }],
    ['link', { rel: 'icon', href: `${base}favicon.svg`, type: 'image/svg+xml' }],
    ['meta', { name: 'theme-color', content: tokens.color.ground.$value }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:site_name', content: 'repomap' }],
    ['meta', { property: 'og:image', content: `${url}og.png` }],
    ['meta', { property: 'og:image:width', content: '1200' }],
    ['meta', { property: 'og:image:height', content: '630' }],
    ['meta', { property: 'og:image:alt', content: 'repomap: a map of your codebase for AI agents' }],
    ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
    ['meta', { name: 'twitter:image', content: `${url}og.png` }],
  ],
  // Each page names its own URL, so search engines index every page, not just the home page.
  transformPageData(pageData) {
    const pageUrl = url + pageData.relativePath.replace(/(^|\/)index\.md$/, '$1').replace(/\.md$/, '')
    const pageTitle = pageData.frontmatter.title ?? (pageData.title || 'repomap: a map of your codebase for AI agents')
    const pageDesc = pageData.frontmatter.description ?? desc
    pageData.frontmatter.head ??= []
    pageData.frontmatter.head.push(
      ['link', { rel: 'canonical', href: pageUrl }],
      ['meta', { property: 'og:url', content: pageUrl }],
      ['meta', { property: 'og:title', content: pageTitle }],
      ['meta', { property: 'og:description', content: pageDesc }],
    )
  },
  themeConfig: {
    logo: { src: '/logo.svg', alt: '' },
    nav: [
      { text: 'Quickstart', link: '/guide/quickstart' },
      { text: 'Tools', link: '/reference/tools' },
      { text: 'Graph UI', link: '/guide/ui' },
      { text: 'Live demo', link: '/demo' },
      { text: 'Benchmarks', link: '/benchmarks' },
      { text: 'npm', link: 'https://www.npmjs.com/package/@sylphx/repomap' },
      { text: 'Recover licence', link: '/recover', target: '_self' },
    ],
    sidebar: [
      { text: 'Guide', items: [
        { text: 'Quickstart', link: '/guide/quickstart' },
        { text: 'Editors and agents', link: '/guide/setup' },
        { text: 'Graph UI and export', link: '/guide/ui' },
        { text: 'Live demo', link: '/demo' },
        { text: 'Claude Code hook', link: '/guide/claude-code-hook' },
        { text: 'Database map', link: '/guide/database' },
        { text: 'Agent-readiness score', link: '/guide/score' },
        { text: 'How it works', link: '/guide/how-it-works' },
        { text: 'From Spine or Locus', link: '/guide/migrate' },
      ] },
      { text: 'Reference', items: [
        { text: 'MCP tools', link: '/reference/tools' },
        { text: 'CLI', link: '/reference/cli' },
      ] },
      { text: 'More', items: [
        { text: 'Benchmarks', link: '/benchmarks' },
        { text: 'Comparison', link: '/compare' },
        { text: 'Vision', link: '/vision' },
        { text: 'Capabilities', link: '/capabilities' },
      ] },
    ],
    socialLinks: [{ icon: 'github', link: 'https://github.com/SylphxAI/repomap' }],
    editLink: { pattern: 'https://github.com/SylphxAI/repomap/edit/main/docs/:path' },
    search: { provider: 'local' },
    lastUpdated: { formatOptions: { dateStyle: 'medium' } },
    sylphx: {
      product: 'repomap',
      license: 'https://github.com/SylphxAI/repomap/blob/main/LICENSE',
      links: [
        { text: 'Quickstart', href: '/guide/quickstart' },
        { text: 'Benchmarks', href: '/benchmarks' },
        { text: 'Changelog', href: 'https://github.com/SylphxAI/repomap/blob/main/CHANGELOG.md' },
        { text: 'GitHub', href: 'https://github.com/SylphxAI/repomap' },
        { text: 'npm', href: 'https://www.npmjs.com/package/@sylphx/repomap' },
      ],
    },
  },
})
