import { defineStore } from 'pinia'

export const DEFAULT_FEATURE_FLAGS = {
	project_background: false,
	page_path: false,
	worlds_tab: false,
	worlds_in_home: true,
	server_project_qa: false,
	show_version_environment_column: false,
	server_ram_as_bytes_always_on: false,
	always_show_app_controls: false,
	skip_non_essential_warnings: false,
	skip_unknown_pack_warning: false,
	pride_fundraiser: true,
	i18n_debug: false,
	show_instance_play_time: true,
	advanced_filters_collapsed: true,
}

export const THEME_OPTIONS = ['purple', 'light', 'dark', 'oled'] as const

// Built-in theme applied under a plugin theme (which only overrides the bits it
// wants) and used as the safe fallback when a selected theme isn't available.
const BASE_THEME = 'dark'

export type FeatureFlag = keyof typeof DEFAULT_FEATURE_FLAGS
export type FeatureFlags = Record<FeatureFlag, boolean>
// Loosened to string so plugin-provided themes (e.g. "Modrinth") are valid too.
export type ColorTheme = string

export type ThemeStore = {
	selectedTheme: string
	// Theme names contributed by enabled plugins (their manifest `theme` value).
	pluginThemes: string[]
	advancedRendering: boolean
	hideNametagSkinsPage: boolean
	toggleSidebar: boolean

	devMode: boolean
	featureFlags: FeatureFlags
}

export const DEFAULT_THEME_STORE: ThemeStore = {
	selectedTheme: 'purple',
	pluginThemes: [],
	advancedRendering: true,
	hideNametagSkinsPage: false,
	toggleSidebar: false,

	devMode: false,
	featureFlags: DEFAULT_FEATURE_FLAGS,
}

export const useTheming = defineStore('themeStore', {
	state: () => DEFAULT_THEME_STORE,
	actions: {
		setThemeState(newTheme: string) {
			// Remember the intent even if the theme isn't available yet (its
			// plugin may load a moment later); setThemeClass falls back visually.
			this.selectedTheme = newTheme
			this.setThemeClass()
		},
		// Registered by the plugin loader on startup once each plugin's `.<name>-mode`
		// CSS has been injected. Re-applies the class so a saved plugin theme resolves.
		setPluginThemes(themes: string[]) {
			// Names become `.<name>-mode` CSS classes, so reject empties and any
			// whitespace (classList would throw on a token containing a space).
			this.pluginThemes = themes.filter(
				(t) => typeof t === 'string' && t.length > 0 && !/\s/.test(t),
			)
			this.setThemeClass()
		},
		setThemeClass() {
			const html = document.getElementsByTagName('html')[0]
			for (const theme of THEME_OPTIONS) {
				html.classList.remove(`${theme}-mode`)
			}
			for (const theme of this.pluginThemes) {
				html.classList.remove(`${theme}-mode`)
			}

			const isBuiltin = (THEME_OPTIONS as readonly string[]).includes(this.selectedTheme)
			const isPlugin = this.pluginThemes.includes(this.selectedTheme)

			if (isBuiltin) {
				html.classList.add(`${this.selectedTheme}-mode`)
			} else if (isPlugin) {
				// Plugin themes layer over the dark base and override only what they
				// redefine (brand colors, etc.), so templates can stay small.
				html.classList.add(`${BASE_THEME}-mode`)
				html.classList.add(`${this.selectedTheme}-mode`)
			} else {
				// Selected theme isn't available (e.g. its plugin is disabled/removed).
				html.classList.add(`${BASE_THEME}-mode`)
			}
		},
		getFeatureFlag(key: FeatureFlag) {
			return this.featureFlags[key] ?? DEFAULT_FEATURE_FLAGS[key]
		},
		getThemeOptions() {
			// Dedupe so a plugin can't inject a duplicate key / built-in collision.
			return [...new Set([...THEME_OPTIONS, ...this.pluginThemes])]
		},
	},
})
