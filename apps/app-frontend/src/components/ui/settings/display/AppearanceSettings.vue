<script setup lang="ts">
import {
	AppearanceSettingsLayout,
	injectAuth,
	injectUserPreferences,
	provideAppearanceSettings,
	useSavable,
} from '@modrinth/ui'
import { computed, inject, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { isPluginTheme, persistPluginTheme } from '@/bytelauncher/plugin-themes'
import {
	type ColorTheme,
	isAccountTheme,
	isBuiltinTheme,
	useTheme,
} from '@/composables/use-theme.ts'
import { type AppSettings, get, set } from '@/helpers/settings.ts'
import { getOS } from '@/helpers/utils'
import { appSettingsModalContextKey } from '@/providers/app-settings-modal'

const theme = useTheme()
const auth = injectAuth()
const { updatePreferences } = injectUserPreferences()
const settingsModal = inject(appSettingsModalContextKey, null)
const os = await getOS()
const settings = ref(await get())

type AppearanceSettingsState = {
	theme: ColorTheme
	syncAcrossDevices: boolean
	advancedRendering: boolean
	nativeDecorations: boolean
}

function getAppearanceSettingsState(settings: AppSettings): AppearanceSettingsState {
	return {
		theme: theme.preferred,
		syncAcrossDevices: settings.sync_theme_across_devices && isAccountTheme(theme.preferred),
		advancedRendering: settings.advanced_rendering,
		nativeDecorations: settings.native_decorations,
	}
}

const { saved, current, changes, saving, hasChanges, reset, save } = useSavable(
	() => getAppearanceSettingsState(settings.value),
	async (appearanceChanges) => {
		const value = current.value
		const savedTheme = isBuiltinTheme(value.theme) ? value.theme : settings.value.theme
		const syncAcrossDevices = value.syncAcrossDevices && isAccountTheme(value.theme)
		if (
			syncAcrossDevices &&
			auth.user.value &&
			(appearanceChanges.theme !== undefined || appearanceChanges.syncAcrossDevices !== undefined)
		) {
			await updatePreferences({
				appearance: value.theme === 'system' ? { auto: true } : { auto: false, theme: value.theme },
			})
		}

		const nextSettings: AppSettings = {
			...settings.value,
			theme: savedTheme,
			sync_theme_across_devices: syncAcrossDevices,
			advanced_rendering: value.advancedRendering,
			native_decorations: value.nativeDecorations,
		}

		await set(nextSettings)
		settings.value = nextSettings
		persistPluginTheme(isPluginTheme(value.theme) ? value.theme : null)
		theme.preferred = value.theme
		theme.syncAcrossDevices = syncAcrossDevices
		theme.advancedRendering = value.advancedRendering
	},
)

const themeOptions = computed(() =>
	theme.options.filter(
		(option) =>
			option !== 'retro' || settings.value.developer_mode || current.value.theme === 'retro',
	),
)

function setTheme(value: ColorTheme): void {
	current.value.theme = value
	if (!isAccountTheme(value)) {
		current.value.syncAcrossDevices = false
	}
}

function setSyncAcrossDevices(enabled: boolean): void {
	current.value.syncAcrossDevices = enabled
}

function setAdvancedRendering(enabled: boolean): void {
	current.value.advancedRendering = enabled
}

function setNativeDecorations(enabled: boolean): void {
	current.value.nativeDecorations = enabled
}

watch(
	[() => current.value.theme, () => saved.value.theme],
	([selectedTheme, savedTheme]) => {
		theme.preview = selectedTheme === savedTheme ? null : selectedTheme
	},
	{ immediate: true },
)

async function saveAppearanceSettings(): Promise<void> {
	try {
		await save()
	} catch {
		return
	}
}

onMounted(() => {
	settingsModal?.registerUnsavedChangesController({
		hasChanges: () => hasChanges.value,
		getOriginal: () => saved.value,
		getModified: () => changes.value,
		isSaving: () => saving.value,
		reset,
		save: saveAppearanceSettings,
	})
})

onBeforeUnmount(() => {
	theme.preview = null
	settingsModal?.registerUnsavedChangesController(null)
})

provideAppearanceSettings({
	deferPersistence: true,
	theme: {
		current: computed(() => current.value.theme),
		options: themeOptions,
		system: computed(() => theme.native),
		set: setTheme,
		syncAcrossDevices: {
			value: computed(() => current.value.syncAcrossDevices),
			set: setSyncAcrossDevices,
		},
		syncDisabled: computed(() => !auth.user.value || !isAccountTheme(current.value.theme)),
	},
	advancedRendering: {
		value: computed(() => current.value.advancedRendering),
		set: setAdvancedRendering,
	},
	nativeDecorations:
		os !== 'MacOS'
			? {
					value: computed(() => current.value.nativeDecorations),
					set: setNativeDecorations,
				}
			: undefined,
	updatePreferences,
})
</script>

<template>
	<AppearanceSettingsLayout />
</template>
