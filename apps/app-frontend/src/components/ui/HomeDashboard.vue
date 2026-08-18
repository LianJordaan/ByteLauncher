<script setup>
import { PlayIcon } from '@modrinth/assets'
import { computed } from 'vue'

import LegacyRowDisplay from '@/bytelauncher/classic-library/LegacyRowDisplay.vue'
import HomeQuickLaunchTile from '@/components/ui/HomeQuickLaunchTile.vue'
import RecentWorldsList from '@/components/ui/world/RecentWorldsList.vue'

const props = defineProps({
	recentInstances: {
		type: Array,
		default: () => [],
	},
	featuredModpacks: {
		type: Array,
		default: () => [],
	},
	featuredMods: {
		type: Array,
		default: () => [],
	},
})

const heroInstances = computed(() => props.recentInstances.slice(0, 4))
const hasFeatured = computed(
	() => (props.featuredModpacks?.length ?? 0) + (props.featuredMods?.length ?? 0) > 0,
)
</script>

<template>
	<div class="flex flex-col gap-6 p-6">
		<h1 v-if="recentInstances.length > 0" class="m-0 text-2xl font-extrabold">Welcome back!</h1>
		<h1 v-else class="m-0 text-2xl font-extrabold">Welcome to ByteLauncher!</h1>

		<section v-if="heroInstances.length > 0" class="flex flex-col gap-3">
			<div class="flex items-center gap-2">
				<PlayIcon class="h-5 w-5 text-brand" />
				<h2 class="m-0 text-lg font-bold text-contrast">Continue playing</h2>
			</div>
			<div class="quick-grid">
				<HomeQuickLaunchTile
					v-for="instance in heroInstances"
					:key="instance.id"
					:instance="instance"
				/>
			</div>
		</section>

		<RecentWorldsList :recent-instances="recentInstances" />

		<LegacyRowDisplay
			v-if="hasFeatured"
			:rows="[
				{
					label: 'Discover a modpack',
					route: '/browse/modpack',
					projects: featuredModpacks,
				},
				{
					label: 'Discover mods',
					route: '/browse/mod',
					projects: featuredMods,
				},
			]"
		/>
	</div>
</template>

<style scoped>
.quick-grid {
	display: grid;
	grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr));
	gap: 0.75rem;
}
</style>
