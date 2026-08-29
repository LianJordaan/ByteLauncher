import { prepareThemeColorTransition } from '@modrinth/ui'
import { computed, reactive, ref, watch } from 'vue'

import {
	getPersistedPluginTheme,
	isPluginTheme,
	pluginThemeNames,
} from '@/bytelauncher/plugin-themes'

export const THEME_OPTIONS = ['dark', 'light', 'purple', 'oled', 'retro', 'system'] as const
export const ACCOUNT_THEME_OPTIONS = ['dark', 'light', 'oled', 'retro', 'system'] as const
export const DARK_THEMES = ['dark', 'purple', 'oled', 'retro'] as const

export type BuiltinColorTheme = (typeof THEME_OPTIONS)[number]
export type AccountColorTheme = (typeof ACCOUNT_THEME_OPTIONS)[number]
export type DarkTheme = (typeof DARK_THEMES)[number]
export type ColorTheme = string
type Theme = string
type NativeTheme = 'light' | 'dark'

const PREFERRED_THEME_KEY = 'modrinth-theme'
const PREFERRED_DARK_THEME_KEY = 'modrinth-preferred-dark-theme'

export function isBuiltinTheme(theme: string): theme is BuiltinColorTheme {
	return (THEME_OPTIONS as readonly string[]).includes(theme)
}

export function isAccountTheme(theme: string): theme is AccountColorTheme {
	return (ACCOUNT_THEME_OPTIONS as readonly string[]).includes(theme)
}

export function isDarkTheme(theme: string): theme is DarkTheme {
	return (DARK_THEMES as readonly string[]).includes(theme)
}

function loadPreferredTheme(): ColorTheme {
	const persistedPluginTheme = getPersistedPluginTheme()
	if (persistedPluginTheme) return persistedPluginTheme

	try {
		const stored = window.localStorage.getItem(PREFERRED_THEME_KEY)
		if (stored && isBuiltinTheme(stored)) {
			return stored
		}
	} catch {
		// storage blocked or full
	}

	for (const option of THEME_OPTIONS) {
		if (option !== 'system' && document.documentElement.classList.contains(`${option}-mode`)) {
			return option
		}
	}

	return 'dark'
}

function loadPreferredDarkTheme(): DarkTheme {
	try {
		const stored = window.localStorage.getItem(PREFERRED_DARK_THEME_KEY)
		if (stored && isDarkTheme(stored)) {
			return stored
		}
	} catch {
		// storage blocked or full
	}

	return 'dark'
}

const preferred = ref<ColorTheme>(loadPreferredTheme())
const preview = ref<ColorTheme | null>(null)
const preferredDark = ref<DarkTheme>(loadPreferredDarkTheme())
const advancedRendering = ref(true)
const syncAcrossDevices = ref(false)
const nativeThemeQuery = window.matchMedia('(prefers-color-scheme: dark)')
const native = ref<NativeTheme>(nativeThemeQuery.matches ? 'dark' : 'light')
const active = computed<Theme>(() => {
	const selectedTheme = preview.value ?? preferred.value
	if (selectedTheme !== 'system') {
		return selectedTheme
	}

	return native.value === 'light' ? 'light' : preferredDark.value
})

nativeThemeQuery.addEventListener('change', (event) => {
	native.value = event.matches ? 'dark' : 'light'
})

watch([preferred, preview], ([selectedPreferred, selectedPreview]) => {
	const selectedTheme = selectedPreview ?? selectedPreferred
	if (isDarkTheme(selectedTheme)) {
		preferredDark.value = selectedTheme
	}
})

watch(
	preferred,
	(theme) => {
		try {
			window.localStorage.setItem(PREFERRED_THEME_KEY, theme)
		} catch {
			// storage blocked or full
		}
	},
	{ immediate: true },
)

watch(preferredDark, (theme) => {
	try {
		window.localStorage.setItem(PREFERRED_DARK_THEME_KEY, theme)
	} catch {
		// storage blocked or full
	}
})

watch(pluginThemeNames, () => {
	const persistedPluginTheme = getPersistedPluginTheme()
	if (persistedPluginTheme && isPluginTheme(persistedPluginTheme)) {
		preferred.value = persistedPluginTheme
	}
})

watch(active, (theme, previousTheme) => {
	if (previousTheme && previousTheme !== theme) {
		prepareThemeColorTransition()
	}
})

const appliedThemeClasses = new Set<string>()

watch(
	[active, pluginThemeNames],
	([theme]) => {
		const html = document.documentElement
		const knownThemeClasses = new Set([
			...appliedThemeClasses,
			...THEME_OPTIONS.map((option) => `${option}-mode`),
			...pluginThemeNames.value.map((option) => `${option}-mode`),
		])
		for (const className of knownThemeClasses) {
			html.classList.remove(className)
		}
		appliedThemeClasses.clear()

		const themes = isBuiltinTheme(theme)
			? [theme]
			: isPluginTheme(theme)
				? ['dark', theme]
				: ['dark']
		for (const selectedTheme of themes) {
			const className = `${selectedTheme}-mode`
			html.classList.add(className)
			appliedThemeClasses.add(className)
		}
	},
	{ immediate: true },
)

function applyAccountAppearance(appearance: { auto: boolean; theme: string }): void {
	if (isDarkTheme(appearance.theme)) {
		preferredDark.value = appearance.theme
	}

	if (appearance.auto) {
		preferred.value = 'system'
		return
	}

	if (isAccountTheme(appearance.theme)) {
		preferred.value = appearance.theme
	}
}

const options = computed<ColorTheme[]>(() => [
	...THEME_OPTIONS,
	...pluginThemeNames.value.filter((theme) => !isBuiltinTheme(theme)),
])

const theme = reactive({
	preferred,
	preview,
	preferredDark,
	active,
	native,
	syncAcrossDevices,
	advancedRendering,
	options,
	applyAccountAppearance,
})

export function useTheme() {
	return theme
}
