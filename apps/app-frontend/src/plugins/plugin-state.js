import { ref } from 'vue'

// Reactive set of enabled plugin ids. Populated by the plugin loader on startup
// and kept in sync when a plugin is toggled in settings. Native, template-driven
// features (e.g. multi-launch) read this to gate themselves reactively.
export const enabledPluginIds = ref(new Set())

export function setEnabledPluginIds(ids) {
	enabledPluginIds.value = new Set(ids)
}

export function setPluginEnabledState(id, enabled) {
	const next = new Set(enabledPluginIds.value)
	if (enabled) {
		next.add(id)
	} else {
		next.delete(id)
	}
	enabledPluginIds.value = next
}
