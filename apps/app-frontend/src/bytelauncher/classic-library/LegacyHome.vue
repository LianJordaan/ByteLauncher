<script setup lang="ts">
import { HomeIcon } from '@modrinth/assets'
import { injectNotificationManager } from '@modrinth/ui'
import type { SearchResult } from '@modrinth/utils'
import dayjs from 'dayjs'
import { computed, ref } from 'vue'

import LegacyRowDisplay from '@/bytelauncher/classic-library/LegacyRowDisplay.vue'
import HomeDashboard from '@/components/ui/HomeDashboard.vue'
import RecentWorldsList from '@/components/ui/world/RecentWorldsList.vue'
import { useAppEvent } from '@/composables/use-app-event'
import { get_search_results } from '@/helpers/cache.js'
import { list } from '@/helpers/instance'
import type { GameInstance } from '@/helpers/types'
import { enabledPluginIds } from '@/plugins/plugin-state'
import { useRootBreadcrumb } from '@/providers/breadcrumbs'

const { handleError } = injectNotificationManager()

useRootBreadcrumb({
	slot: 'root',
	id: 'home',
	label: 'Home',
	to: '/',
	visual: { type: 'icon', component: HomeIcon },
})

const instances = ref<GameInstance[]>([])
const featuredModpacks = ref<SearchResult[]>([])
const featuredMods = ref<SearchResult[]>([])
const installedModpacksFilter = ref('')
const dashboardEnabled = computed(() => enabledPluginIds.value.has('experimental-home'))
const recentInstances = computed(() =>
	instances.value
		.filter((instance) => instance.last_played)
		.slice()
		.sort((a, b) => dayjs(b.last_played).diff(dayjs(a.last_played))),
)
const hasFeaturedProjects = computed(
	() => (featuredModpacks.value.length ?? 0) + (featuredMods.value.length ?? 0) > 0,
)

async function fetchInstances() {
	instances.value = await list().catch((error) => {
		handleError(error)
		return []
	})
	installedModpacksFilter.value = instances.value
		.flatMap((instance) =>
			instance.link?.project_id ? [`NOT\"project_id\"=\"${instance.link.project_id}\"`] : [],
		)
		.join(' AND ')
}

async function fetchFeaturedProjects() {
	const [modpacks, mods] = await Promise.all([
		get_search_results(
			`?facets=[[\"project_type:modpack\"]]&limit=10&index=follows&filters=${installedModpacksFilter.value}`,
		),
		get_search_results('?facets=[[\"project_type:mod\"]]&limit=10&index=follows'),
	])
	featuredModpacks.value = modpacks?.result.hits ?? []
	featuredMods.value = mods?.result.hits ?? []
}

await fetchInstances()
await fetchFeaturedProjects()

useAppEvent('instance', async (event) => {
	await fetchInstances()
	if (event.event === 'created' || event.event === 'removed') {
		await fetchFeaturedProjects()
	}
})
</script>

<template>
	<HomeDashboard
		v-if="dashboardEnabled"
		:recent-instances="recentInstances"
		:featured-modpacks="featuredModpacks"
		:featured-mods="featuredMods"
	/>
	<div v-else class="p-6 flex flex-col gap-2">
		<h1 v-if="recentInstances.length > 0" class="m-0 text-2xl font-extrabold">Welcome back!</h1>
		<h1 v-else class="m-0 text-2xl font-extrabold">Welcome to ByteLauncher!</h1>
		<RecentWorldsList :recent-instances="recentInstances" />
		<LegacyRowDisplay
			v-if="hasFeaturedProjects"
			:rows="[
				{ label: 'Discover a modpack', route: '/browse/modpack', projects: featuredModpacks },
				{ label: 'Discover mods', route: '/browse/mod', projects: featuredMods },
			]"
		/>
	</div>
</template>
