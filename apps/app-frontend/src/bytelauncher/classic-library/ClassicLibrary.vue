<script setup lang="ts">
import { LibraryIcon, PlusIcon } from '@modrinth/assets'
import { Button, defineMessages, injectNotificationManager, NavTabs, useVIntl } from '@modrinth/ui'
import { useEventListener } from '@vueuse/core'
import { computed, inject, ref, shallowRef, watch, watchEffect } from 'vue'
import { useRouter } from 'vue-router'

import { NewInstanceImage } from '@/assets/icons'
import { CLASSIC_LIBRARY_PLUGIN_ID } from '@/bytelauncher/native-plugins'
import { useAppEvent } from '@/composables/use-app-event'
import { get_project_v3_many } from '@/helpers/cache.js'
import { list } from '@/helpers/instance'
import type { GameInstance } from '@/helpers/types'
import { enabledPluginIds, pluginsReady } from '@/plugins/plugin-state'
import { useRootBreadcrumb } from '@/providers/breadcrumbs'

const router = useRouter()
const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const showCreationModal = inject<() => void>('showCreationModal')
const messages = defineMessages({
	library: { id: 'bytelauncher.classic-library.title', defaultMessage: 'Library' },
	all: { id: 'bytelauncher.classic-library.tab.all', defaultMessage: 'All instances' },
	modpacks: { id: 'bytelauncher.classic-library.tab.modpacks', defaultMessage: 'Modpacks' },
	servers: { id: 'bytelauncher.classic-library.tab.servers', defaultMessage: 'Servers' },
	custom: { id: 'bytelauncher.classic-library.tab.custom', defaultMessage: 'Custom' },
	newInstance: { id: 'bytelauncher.classic-library.new-instance', defaultMessage: 'New instance' },
	noInstances: { id: 'bytelauncher.classic-library.empty', defaultMessage: 'No instances found' },
	createInstance: {
		id: 'bytelauncher.classic-library.create-instance',
		defaultMessage: 'Create new instance',
	},
})

useRootBreadcrumb({
	slot: 'root',
	id: 'library',
	label: formatMessage(messages.library),
	to: '/library',
	visual: { type: 'icon', component: LibraryIcon },
})

const classicLibraryEnabled = computed(() => enabledPluginIds.value.has(CLASSIC_LIBRARY_PLUGIN_ID))
watch(
	[pluginsReady, classicLibraryEnabled],
	([ready, enabled]) => {
		if (ready && !enabled) void router.replace('/')
	},
	{ immediate: true },
)

const instances = shallowRef<GameInstance[]>([])
const serverProjectIds = ref(new Set<string>())
const offline = ref(!navigator.onLine)
let latestFetch = 0

useEventListener(window, 'offline', () => (offline.value = true))
useEventListener(window, 'online', () => (offline.value = false))

async function fetchInstances() {
	const fetchId = ++latestFetch
	try {
		const nextInstances = await list()
		if (fetchId === latestFetch) instances.value = nextInstances
	} catch (error) {
		if (fetchId === latestFetch) handleError(error)
	}
}

watchEffect(async () => {
	const ids = [
		...new Set(
			instances.value.flatMap((instance) =>
				instance.link?.project_id ? [instance.link.project_id] : [],
			),
		),
	]
	if (ids.length === 0) {
		serverProjectIds.value = new Set()
		return
	}
	try {
		const projects = await get_project_v3_many(ids, 'must_revalidate')
		serverProjectIds.value = new Set(
			projects.filter((project) => project?.minecraft_server != null).map((project) => project.id),
		)
	} catch {
		serverProjectIds.value = new Set()
	}
})

await fetchInstances()
useAppEvent('instance', fetchInstances)
useAppEvent('instance_groups_changed', fetchInstances)
</script>

<template>
	<div v-if="pluginsReady && classicLibraryEnabled" class="p-6 flex flex-col gap-3">
		<div class="flex items-center justify-between gap-3">
			<NavTabs
				:links="[
					{ label: formatMessage(messages.all), href: '/library' },
					{ label: formatMessage(messages.modpacks), href: '/library/modpacks' },
					{ label: formatMessage(messages.servers), href: '/library/servers' },
					{ label: formatMessage(messages.custom), href: '/library/custom' },
				]"
			/>
			<Button type="colored" color="brand" :disabled="offline" @click="showCreationModal?.()">
				<PlusIcon />
				{{ formatMessage(messages.newInstance) }}
			</Button>
		</div>
		<RouterView
			v-if="instances.length > 0"
			:instances="instances"
			:server-project-ids="serverProjectIds"
		/>
		<div v-else class="flex flex-col items-center justify-center gap-3 py-16">
			<NewInstanceImage class="h-40 w-40" />
			<h3 class="m-0">{{ formatMessage(messages.noInstances) }}</h3>
			<Button type="colored" color="brand" :disabled="offline" @click="showCreationModal?.()">
				<PlusIcon />
				{{ formatMessage(messages.createInstance) }}
			</Button>
		</div>
	</div>
</template>
