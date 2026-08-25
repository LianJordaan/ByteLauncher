import { computed, reactive, ref, watch } from 'vue'

import {
	getPersistedPluginTheme,
	isPluginTheme,
	pluginThemeNames,
} from '@/bytelauncher/plugin-themes'

export const THEME_OPTIONS = ['dark', 'light', 'purple', 'oled', 'retro', 'system'] as const

export type BuiltinColorTheme = (typeof THEME_OPTIONS)[number]
export type ColorTheme = string
type Theme = string

const preferred = ref<ColorTheme>(getPersistedPluginTheme() ?? 'dark')
const preview = ref<ColorTheme | null>(null)
const advancedRendering = ref(true)
const syncAcrossDevices = ref(false)
const nativeThemeQuery = window.matchMedia('(prefers-color-scheme: dark)')
const native = ref<Theme>(nativeThemeQuery.matches ? 'dark' : 'light')
const active = computed<Theme>(() => {
	const selectedTheme = preview.value ?? preferred.value
	return selectedTheme === 'system' ? native.value : selectedTheme
})

nativeThemeQuery.addEventListener('change', (event) => {
	native.value = event.matches ? 'dark' : 'light'
})

const appliedThemeClasses = new Set<string>()

export function isBuiltinTheme(theme: string): theme is BuiltinColorTheme {
	return (THEME_OPTIONS as readonly string[]).includes(theme)
}

watch(
	[active, pluginThemeNames],
	([theme]) => {
		const html = document.documentElement
		for (const className of appliedThemeClasses) {
			html.classList.remove(className)
		}
		appliedThemeClasses.clear()

		const themes = isPluginTheme(theme) ? ['dark', theme] : [isBuiltinTheme(theme) ? theme : 'dark']
		for (const selectedTheme of themes) {
			const className = `${selectedTheme}-mode`
			html.classList.add(className)
			appliedThemeClasses.add(className)
		}
	},
	{ immediate: true },
)

const options = computed<ColorTheme[]>(() => [
	...THEME_OPTIONS,
	...pluginThemeNames.value.filter((theme) => !isBuiltinTheme(theme)),
])

const theme = reactive({
	preferred,
	preview,
	active,
	native,
	syncAcrossDevices,
	advancedRendering,
	options,
})

export function useTheme() {
	return theme
}
