import { defineConfig } from 'vitepress'

const url = 'https://sylphxai.github.io/repomap/'
const desc = 'A map of your codebase for AI agents: code graph, search, call paths and change impact, with an interactive graph UI. Local, no API key, MIT.'

export default defineConfig({
  base: '/repomap/',
  title: 'repomap',
  description: desc,
  appearance: 'force-dark',
  cleanUrls: true,
  lastUpdated: true,
  sitemap: { hostname: url },
  head: [
    ['link', { rel: 'icon', href: "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'%3E%3Ccircle cx='8' cy='9' r='4' fill='%238aa4ff'/%3E%3Ccircle cx='24' cy='8' r='3' fill='%23ff7eb6'/%3E%3Ccircle cx='21' cy='23' r='5' fill='%2342d6a4'/%3E%3Ccircle cx='7' cy='24' r='3' fill='%23ffb454'/%3E%3C/svg%3E" }],
    ['meta', { name: 'theme-color', content: '#06080c' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:site_name', content: 'repomap' }],
    ['meta', { property: 'og:image', content: `${url}img/hero-excalidraw.webp` }],
    ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
    ['meta', { name: 'twitter:image', content: `${url}img/hero-excalidraw.webp` }],
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
    logo: { src: '/logo.svg', alt: 'repomap' },
    nav: [
      { text: 'Quickstart', link: '/guide/quickstart' },
      { text: 'Tools', link: '/reference/tools' },
      { text: 'Graph UI', link: '/guide/ui' },
      { text: 'Live demo', link: '/demo' },
      { text: 'Benchmarks', link: '/benchmarks' },
      { text: 'npm', link: 'https://www.npmjs.com/package/@sylphx/repomap' },
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
      ] },
    ],
    socialLinks: [{ icon: 'github', link: 'https://github.com/SylphxAI/repomap' }],
    editLink: { pattern: 'https://github.com/SylphxAI/repomap/edit/main/docs/:path' },
    search: { provider: 'local' },
    footer: { message: 'MIT licensed · local, no API key', copyright: '© Sylphx' },
  },
})
