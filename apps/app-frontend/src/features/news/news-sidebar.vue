<script setup lang="ts">
import { EyeIcon, EyeOffIcon, NewspaperIcon } from '@modrinth/assets'
import {
	ButtonLink,
	defineMessages,
	type NewsArticle,
	NewsArticleCard,
	useVIntl,
} from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, watch } from 'vue'

import {
	articleId,
	hiddenNewsCount,
	hideAllNews,
	hideNews,
	isNewsHidden,
	setShowHiddenNews,
	showHiddenNews,
	unhideNews,
} from '@/plugins/hidden-news'
import { enabledPluginIds } from '@/plugins/plugin-state'

import { newsArticlesQueryOptions } from './queries'

const { formatMessage } = useVIntl()
const messages = defineMessages({
	news: {
		id: 'app.news.title',
		defaultMessage: 'News',
	},
	viewAllNews: {
		id: 'app.news.view-all',
		defaultMessage: 'View all news',
	},
})

const { data: news, error } = useQuery(newsArticlesQueryOptions())
const hideNewsEnabled = computed(() => enabledPluginIds.value.has('hide-news'))
const hideNewsSection = computed(() => hideNewsEnabled.value && hideAllNews.value)
const displayedNews = computed(() => {
	if (!news.value) return []
	if (!hideNewsEnabled.value) return news.value.slice(0, 4)
	const visible = news.value.filter((article) => !isNewsHidden(article)).slice(0, 4)
	if (!showHiddenNews.value) return visible
	const visibleIds = new Set(visible.map(articleId))
	return [
		...visible,
		...news.value.filter((article) => isNewsHidden(article) && !visibleIds.has(articleId(article))),
	]
})

function handleNewsContextMenu(event: MouseEvent, article: NewsArticle) {
	if (!hideNewsEnabled.value || isNewsHidden(article)) return
	event.preventDefault()
	event.stopPropagation()
	hideNews(article)
}

watch(error, (error) => {
	if (error) console.error('Failed to fetch news articles', error)
})
</script>

<template>
	<div v-if="news && news.length > 0 && !hideNewsSection" class="p-4 flex flex-col items-center">
		<div class="mb-4 flex w-full items-center justify-between gap-2">
			<h3 class="m-0 text-base font-medium text-primary">
				{{ formatMessage(messages.news) }}
			</h3>
			<button
				v-if="hideNewsEnabled && hiddenNewsCount > 0"
				type="button"
				class="flex items-center gap-1 rounded-md border-0 bg-transparent p-1 text-secondary hover:text-primary"
				:aria-label="showHiddenNews ? 'Hide hidden articles' : 'Review hidden articles'"
				:title="showHiddenNews ? 'Hide hidden articles' : 'Review hidden articles'"
				@click="setShowHiddenNews(!showHiddenNews)"
			>
				<EyeOffIcon v-if="showHiddenNews" class="size-4" />
				<EyeIcon v-else class="size-4" />
				<span class="text-xs">{{ hiddenNewsCount }}</span>
			</button>
		</div>
		<div class="space-y-4 flex flex-col items-center w-full">
			<template v-for="(item, index) in displayedNews" :key="articleId(item) || `news-${index}`">
				<NewsArticleCard v-if="!hideNewsEnabled" :article="item" />
				<div
					v-else
					class="relative w-full"
					:class="{ 'opacity-50': isNewsHidden(item) }"
					@contextmenu="(event) => handleNewsContextMenu(event, item)"
				>
					<NewsArticleCard :article="item" />
					<button
						v-if="isNewsHidden(item)"
						type="button"
						class="absolute top-2 right-2 rounded-md border-0 bg-bg-raised px-2 py-1 text-xs text-primary shadow-sm"
						@click.prevent.stop="unhideNews(item)"
					>
						Restore
					</button>
				</div>
			</template>
			<ButtonLink
				type="colored"
				color="brand"
				size="xl"
				href="https://modrinth.com/news"
				target="_blank"
				class="my-4"
			>
				<NewspaperIcon />
				{{ formatMessage(messages.viewAllNews) }}
			</ButtonLink>
		</div>
	</div>
</template>
