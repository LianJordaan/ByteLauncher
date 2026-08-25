import { ref } from 'vue'

const PLUGIN_THEME_STORAGE_KEY = 'bytelauncher-plugin-theme'

export const pluginThemeNames = ref<string[]>([])

function isValidThemeName(theme: unknown): theme is string {
	return typeof theme === 'string' && theme.length > 0 && !/\s/.test(theme)
}

export function registerPluginThemes(themes: unknown[]) {
	pluginThemeNames.value = [...new Set(themes.filter(isValidThemeName))]
}

export function isPluginTheme(theme: string) {
	return pluginThemeNames.value.includes(theme)
}

export function getPersistedPluginTheme(): string | null {
	try {
		return localStorage.getItem(PLUGIN_THEME_STORAGE_KEY)
	} catch {
		return null
	}
}

export function persistPluginTheme(theme: string | null) {
	try {
		if (theme) localStorage.setItem(PLUGIN_THEME_STORAGE_KEY, theme)
		else localStorage.removeItem(PLUGIN_THEME_STORAGE_KEY)
	} catch {
		// Theme selection still applies for the current session.
	}
}
