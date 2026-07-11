<script setup lang="ts">
import { ButtonStyled, Toggle } from '@modrinth/ui'
import { invoke } from '@tauri-apps/api/core'
import { ref } from 'vue'

import { openPath, restartApp } from '@/helpers/utils.js'

interface PluginData {
	id: string
	name: string
	description: string
	version: string
	author: string
	enabled: boolean
	builtin: boolean
	js: string | null
	css: string | null
}

const plugins = ref<PluginData[]>((await invoke('plugin:addons|read_plugins')) as PluginData[])
const pluginsDir = ref<string>((await invoke('plugin:addons|get_plugins_dir')) as string)
const restartNeeded = ref(false)

async function toggle(plugin: PluginData) {
	const next = !plugin.enabled
	plugin.enabled = next
	restartNeeded.value = true
	try {
		await invoke('plugin:addons|set_plugin_enabled', { id: plugin.id, enabled: next })
	} catch (e) {
		console.error('Failed to toggle plugin', e)
		plugin.enabled = !next
	}
}

async function openFolder() {
	if (pluginsDir.value) {
		await openPath(pluginsDir.value)
	}
}
</script>

<template>
	<div class="flex flex-col gap-4 min-w-[600px]">
		<div class="flex items-center justify-between gap-4">
			<div>
				<h2 class="m-0 text-lg font-semibold text-contrast">Plugins</h2>
				<p class="m-0 mt-1 text-sm">
					Toggle plugins on or off. Plugins live in your plugins folder — add your own by dropping
					in a folder containing a <code>manifest.json</code> with its <code>.js</code>/<code>.css</code>.
				</p>
			</div>
			<ButtonStyled>
				<button @click="openFolder">Open plugins folder</button>
			</ButtonStyled>
		</div>

		<div v-if="restartNeeded" class="flex items-center justify-between gap-4">
			<p class="m-0 text-sm">Restart the app to apply your changes.</p>
			<ButtonStyled color="brand">
				<button @click="restartApp">Restart now</button>
			</ButtonStyled>
		</div>

		<div
			v-for="plugin in plugins"
			:key="plugin.id"
			class="flex items-center justify-between gap-4"
		>
			<div>
				<h2 class="m-0 text-lg font-semibold text-contrast">
					{{ plugin.name }}
					<span v-if="plugin.builtin" class="text-sm font-normal">(built-in)</span>
				</h2>
				<p class="m-0 mt-1 text-sm">{{ plugin.description }}</p>
				<p v-if="plugin.version || plugin.author" class="m-0 mt-1 text-sm">
					<span v-if="plugin.version">v{{ plugin.version }}</span>
					<span v-if="plugin.author"> · {{ plugin.author }}</span>
				</p>
			</div>
			<Toggle
				:id="`plugin-${plugin.id}`"
				:model-value="plugin.enabled"
				@update:model-value="() => toggle(plugin)"
			/>
		</div>

		<p v-if="plugins.length === 0" class="m-0 text-sm">No plugins found.</p>
	</div>
</template>
