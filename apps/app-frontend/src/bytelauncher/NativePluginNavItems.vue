<script setup lang="ts">
import { LibraryIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'
import { useRoute } from 'vue-router'

import { CLASSIC_LIBRARY_PLUGIN_ID } from '@/bytelauncher/native-plugins'
import NavButton from '@/components/ui/NavButton.vue'
import { enabledPluginIds, pluginsReady } from '@/plugins/plugin-state'

const route = useRoute()
const { formatMessage } = useVIntl()
const messages = defineMessages({
	library: { id: 'bytelauncher.classic-library.navigation', defaultMessage: 'Library' },
})
const classicLibraryEnabled = computed(
	() => pluginsReady.value && enabledPluginIds.value.has(CLASSIC_LIBRARY_PLUGIN_ID),
)
</script>

<template>
	<NavButton
		v-if="classicLibraryEnabled"
		v-tooltip.right="formatMessage(messages.library)"
		to="/library"
		:is-primary="(currentRoute) => currentRoute.path.startsWith('/library')"
		:is-subpage="
			() =>
				route.path.startsWith('/instance') ||
				((route.path.startsWith('/browse') || route.path.startsWith('/project')) && route.query.i)
		"
	>
		<LibraryIcon />
	</NavButton>
</template>
