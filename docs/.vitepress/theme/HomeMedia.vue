<script setup lang="ts">
// Home: the product visual between the hero and the feature grid.
// Reads frontmatter.media: { kind: 'video' | 'image', src, poster?, alt, caption?, link?, linkText? }.
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useData, withBase } from 'vitepress'

const { frontmatter } = useData()
const m = computed(() => frontmatter.value.media)
const u = (s?: string) => (s && s.startsWith('/') ? withBase(s) : s)
const video = ref<HTMLVideoElement | null>(null)
const playback = ref<'idle' | 'auto' | 'manual'>('idle')
let observer: IntersectionObserver | undefined

onMounted(() => {
  if (!video.value) return
  const admit = () => {
    const network = (navigator as Navigator & { connection?: { saveData?: boolean } }).connection
    const reduced = network?.saveData === true || matchMedia('(prefers-reduced-motion: reduce)').matches ||
      matchMedia('(prefers-reduced-data: reduce)').matches
    playback.value = reduced ? 'manual' : 'auto'
  }
  // Omit src from HTML: autoplay can fetch even with preload="none".
  if (!('IntersectionObserver' in window)) {
    playback.value = 'manual'
    return
  }
  observer = new IntersectionObserver(([entry]) => {
    if (!entry?.isIntersecting || entry.intersectionRatio === 0) return
    observer?.disconnect()
    admit()
  })
  observer.observe(video.value)
})
onBeforeUnmount(() => observer?.disconnect())
</script>

<template>
  <figure v-if="m" class="sx-media">
    <video v-if="m.kind === 'video'" ref="video" :poster="u(m.poster)" :autoplay="playback === 'auto'"
      :controls="playback === 'manual'" :src="playback === 'idle' ? undefined : u(m.src)" loop muted playsinline preload="none"
      :aria-label="m.alt" width="1600" height="1000" />
    <img v-else :src="u(m.src)" :alt="m.alt" width="1400" height="780" decoding="async" fetchpriority="high" />
    <figcaption v-if="m.caption">
      {{ m.caption }}
      <a v-if="m.link" :href="u(m.link)">{{ m.linkText }}</a>
    </figcaption>
  </figure>
</template>
