<script setup lang="ts">
import { ServerStackIcon } from '@modrinth/assets'
import { computed } from 'vue'

import { hostingPanelUrl } from '@/plugins/hosting-panel'
import { enabledPluginIds } from '@/plugins/plugin-state'

const hostingEnabled = computed(() => enabledPluginIds.value.has('hosting'))
</script>

<template>
	<iframe
		v-if="hostingEnabled"
		:key="hostingPanelUrl"
		:src="hostingPanelUrl"
		class="block h-full w-full border-0"
		title="ByteBuilders Hosting"
		allow="clipboard-read; clipboard-write; fullscreen"
	/>
	<div
		v-else
		class="flex h-full w-full flex-col items-center justify-center gap-4 px-6 text-center"
	>
		<div class="flex h-20 w-20 items-center justify-center rounded-2xl bg-brand-highlight text-brand">
			<ServerStackIcon class="h-10 w-10" />
		</div>
		<h1 class="m-0 text-3xl font-extrabold text-contrast">ByteBuilders Hosting</h1>
		<p class="m-0 text-lg font-semibold text-brand">Experimental</p>
		<p class="m-0 max-w-md text-secondary">
			Turn on the <strong>ByteBuilders Hosting</strong> plugin in Settings → Plugins to open the
			panel right here. You can pick which panel to use in that plugin's settings.
		</p>
	</div>
</template>
