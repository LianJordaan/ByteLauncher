<script setup lang="ts">
import { openUrl } from '@tauri-apps/plugin-opener'

const PANEL_URL = 'https://panel.bytebuilders.co.za'
</script>

<template>
	<div class="flex h-full w-full flex-col">
		<div
			class="flex items-center justify-between gap-2 border-0 border-b border-solid border-surface-5 bg-surface-2 px-4 py-2"
		>
			<span class="text-sm font-semibold text-contrast">ByteBuilders Hosting</span>
			<button
				class="cursor-pointer border-none bg-transparent text-sm font-semibold text-brand hover:underline"
				@click="openUrl(PANEL_URL)"
			>
				Open in browser ↗
			</button>
		</div>
		<iframe
			:src="PANEL_URL"
			class="min-h-0 w-full flex-1 border-none"
			title="ByteBuilders Hosting"
			allow="clipboard-read; clipboard-write; fullscreen"
		/>
	</div>
</template>
