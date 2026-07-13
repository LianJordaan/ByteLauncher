<script setup>
import { EyeIcon, EyeOffIcon, HistoryIcon, NewspaperIcon } from '@modrinth/assets'
import { ButtonStyled, NewsArticleCard } from '@modrinth/ui'
import { computed, ref } from 'vue'

import ContextMenu from '@/components/ui/ContextMenu.vue'
import {
	articleId,
	hiddenNewsCount,
	hideAllNews,
	hideNews,
	isNewsHidden,
	resetHiddenNews,
	setShowHiddenNews,
	showHiddenNews,
	unhideNews,
} from '@/plugins/hidden-news'
import { enabledPluginIds } from '@/plugins/plugin-state'

const props = defineProps({
	news: {
		type: Array,
		default: () => [],
	},
})

const articleMenu = ref(null)

const hideNewsEnabled = computed(() => enabledPluginIds.value.has('hide-news'))

// "Hide all news" option: with the plugin on, hide the whole section.
const hideEntireNews = computed(() => hideNewsEnabled.value && hideAllNews.value)

// Which articles to render. With the plugin off, this is the original top 4.
// With it on, hidden articles are filtered out and the list backfills from the
// feed. In "show hidden" mode we additionally surface every hidden article that
// is still in the feed so it can be restored.
const displayedNews = computed(() => {
	const all = props.news ?? []
	if (!hideNewsEnabled.value) return all.slice(0, 4)

	const visible = all.filter((a) => !isNewsHidden(a)).slice(0, 4)
	if (!showHiddenNews.value) return visible

	const keep = new Set([...visible, ...all.filter((a) => isNewsHidden(a))].map(articleId))
	return all.filter((a) => keep.has(articleId(a)))
})

function showArticleMenu(event, article) {
	if (!hideNewsEnabled.value) return
	event.preventDefault()
	event.stopPropagation()
	const option = isNewsHidden(article) ? { name: 'unhide', color: 'primary' } : { name: 'hide' }
	articleMenu.value?.showMenu(event, article, [option])
}

function onMenuOption({ item, option }) {
	if (option === 'hide') hideNews(item)
	else if (option === 'unhide') unhideNews(item)
}

function toggleShowHidden() {
	setShowHiddenNews(!showHiddenNews.value)
}
</script>

<template>
	<div v-if="news && news.length > 0 && !hideEntireNews" class="p-4 flex flex-col items-center">
		<div class="mb-4 flex w-full items-center justify-between gap-2">
			<h3 class="m-0 text-base text-primary font-medium">News</h3>
			<div v-if="hideNewsEnabled && hiddenNewsCount > 0" class="flex items-center gap-1">
				<button
					v-tooltip="showHiddenNews ? 'Stop showing hidden' : `Show ${hiddenNewsCount} hidden`"
					class="news-tool-btn"
					@click="toggleShowHidden"
				>
					<EyeOffIcon v-if="showHiddenNews" class="h-4 w-4" />
					<EyeIcon v-else class="h-4 w-4" />
					<span class="text-xs font-semibold">{{ hiddenNewsCount }}</span>
				</button>
				<button
					v-tooltip="'Restore all hidden articles'"
					class="news-tool-btn"
					@click="resetHiddenNews"
				>
					<HistoryIcon class="h-4 w-4" />
				</button>
			</div>
		</div>
		<div class="space-y-4 flex flex-col items-center w-full">
			<div
				v-for="(item, index) in displayedNews"
				:key="`news-${articleId(item) || index}`"
				class="relative w-full"
				:class="{ 'opacity-40 transition-opacity hover:opacity-75': isNewsHidden(item) }"
				@contextmenu="showArticleMenu($event, item)"
			>
				<NewsArticleCard :article="item" />
				<button
					v-if="isNewsHidden(item)"
					v-tooltip="'Unhide this article'"
					class="unhide-badge"
					@click.prevent.stop="unhideNews(item)"
				>
					<EyeIcon class="h-3.5 w-3.5" /> Unhide
				</button>
			</div>
			<ButtonStyled color="brand" size="large">
				<a href="https://modrinth.com/news" target="_blank" class="my-4">
					<NewspaperIcon /> View all news
				</a>
			</ButtonStyled>
		</div>
		<ContextMenu ref="articleMenu" @option-clicked="onMenuOption">
			<template #hide><EyeOffIcon /> Hide this article</template>
			<template #unhide><EyeIcon /> Unhide this article</template>
		</ContextMenu>
	</div>
</template>

<style scoped>
.news-tool-btn {
	display: flex;
	align-items: center;
	gap: 0.25rem;
	padding: 0.25rem 0.4rem;
	border: none;
	border-radius: var(--radius-sm);
	background: var(--color-button-bg);
	color: var(--color-base);
	cursor: pointer;
	transition: filter 0.1s ease-in-out;
}

.news-tool-btn:hover {
	filter: brightness(0.85);
	color: var(--color-contrast);
}

.unhide-badge {
	position: absolute;
	top: 0.5rem;
	right: 0.5rem;
	display: flex;
	align-items: center;
	gap: 0.25rem;
	padding: 0.2rem 0.5rem;
	font-size: 0.75rem;
	font-weight: 600;
	border: none;
	border-radius: var(--radius-md);
	background: var(--color-brand);
	color: #ffffff;
	cursor: pointer;
	box-shadow: var(--shadow-floating);
}

.unhide-badge:hover {
	filter: brightness(1.1);
}
</style>
