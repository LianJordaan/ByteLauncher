<script setup lang="ts">
import { computed } from 'vue'

import LegacyHome from '@/bytelauncher/classic-library/LegacyHome.vue'
import { CLASSIC_LIBRARY_PLUGIN_ID } from '@/bytelauncher/native-plugins'
import UpstreamHome from '@/pages/Index.vue'
import { enabledPluginIds, pluginsReady } from '@/plugins/plugin-state'

defineOptions({ name: 'LibraryPage' })

const useLegacyHome = computed(
	() =>
		enabledPluginIds.value.has(CLASSIC_LIBRARY_PLUGIN_ID) ||
		enabledPluginIds.value.has('experimental-home'),
)
</script>

<template>
	<LegacyHome v-if="pluginsReady && useLegacyHome" />
	<UpstreamHome v-else-if="pluginsReady" />
</template>
