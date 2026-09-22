import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Spine',
  description: 'Repository architecture with file-level proof',
  head: [
    ['link', { rel: 'canonical', href: 'https://sylphxai.github.io/spine/' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:title', content: 'Spine — Repository architecture with file-level proof' }],
    ['meta', { property: 'og:description', content: 'Understand a repo, trace behavior, and estimate change impact with file-level evidence.' }],
    ['meta', { property: 'og:url', content: 'https://sylphxai.github.io/spine/' }],
    ['meta', { name: 'twitter:card', content: 'summary' }]
  ],
  cleanUrls: true,
  themeConfig: {
    nav: [
      { text: 'Quickstart', link: '/guide/quickstart' },
      { text: 'Tools', link: '/reference/tools' },
      { text: 'GitHub', link: 'https://github.com/SylphxAI/spine' },
      { text: 'npm', link: 'https://www.npmjs.com/package/@sylphx/spine' }
    ],
    sidebar: [
      { text: 'Guide', items: [{ text: 'Quickstart', link: '/guide/quickstart' }, { text: 'Predictable defaults', link: '/reference/defaults' }] },
      { text: 'Reference', items: [{ text: 'Tools', link: '/reference/tools' }] }
    ],
    socialLinks: [{ icon: 'github', link: 'https://github.com/SylphxAI/spine' }],
    footer: { message: 'Spine · local-first agent tooling · MIT' }
  }
})
