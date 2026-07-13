import { computed, ref } from 'vue'

// State for the built-in "Hide News" plugin. Hidden articles are stored in
// localStorage — never in the shared Modrinth app.db — so this never touches
// the database the real Modrinth App reads. Each entry is { id, title } where
// id is the article's link/path (stable + unique per article).

const STORAGE_KEY = 'bytelauncher-hidden-news'
const SHOW_HIDDEN_KEY = 'bytelauncher-show-hidden-news'

function loadHidden() {
	try {
		const raw = localStorage.getItem(STORAGE_KEY)
		if (!raw) return []
		const parsed = JSON.parse(raw)
		return Array.isArray(parsed) ? parsed.filter((e) => e && typeof e.id === 'string') : []
	} catch {
		return []
	}
}

export const hiddenNews = ref(loadHidden())
export const showHiddenNews = ref(localStorage.getItem(SHOW_HIDDEN_KEY) === '1')

export const hiddenNewsCount = computed(() => hiddenNews.value.length)

function persist() {
	try {
		localStorage.setItem(STORAGE_KEY, JSON.stringify(hiddenNews.value))
	} catch (e) {
		console.error('[hide-news] failed to save hidden articles', e)
	}
}

// The stable identity of an article. In this app articles carry both `path`
// and `link` (set equal), so either works; `title` is a last-resort fallback.
export function articleId(article) {
	return article?.path ?? article?.link ?? article?.title ?? ''
}

export function isNewsHidden(article) {
	const id = articleId(article)
	return id !== '' && hiddenNews.value.some((e) => e.id === id)
}

export function hideNews(article) {
	const id = articleId(article)
	if (id === '' || hiddenNews.value.some((e) => e.id === id)) return
	hiddenNews.value = [...hiddenNews.value, { id, title: article?.title ?? id }]
	persist()
}

export function unhideNews(articleOrId) {
	const id = typeof articleOrId === 'string' ? articleOrId : articleId(articleOrId)
	if (id === '') return
	hiddenNews.value = hiddenNews.value.filter((e) => e.id !== id)
	persist()
}

export function resetHiddenNews() {
	hiddenNews.value = []
	persist()
}

export function setShowHiddenNews(value) {
	showHiddenNews.value = value
	try {
		localStorage.setItem(SHOW_HIDDEN_KEY, value ? '1' : '0')
	} catch (e) {
		console.error('[hide-news] failed to save show-hidden preference', e)
	}
}
