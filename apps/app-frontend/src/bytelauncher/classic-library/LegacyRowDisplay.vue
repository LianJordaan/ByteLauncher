<script setup lang="ts">
import { HeadingLink } from '@modrinth/ui'

import LegacyProjectCard from '@/components/ui/LegacyProjectCard.vue'

defineProps<{
	rows: Array<{
		label: string
		route: string
		projects: Array<Record<string, unknown> & { project_id?: string }>
	}>
}>()
</script>

<template>
	<div class="flex flex-col gap-4">
		<div
			v-for="row in rows"
			:key="row.label"
			class="flex min-w-0 flex-col items-start overflow-hidden"
		>
			<HeadingLink class="mt-1" :to="row.route">{{ row.label }}</HeadingLink>
			<section class="project-grid">
				<LegacyProjectCard
					v-for="project in row.projects.slice(0, 10)"
					:key="project?.project_id"
					class="w-full max-w-full"
					:project="project"
				/>
			</section>
		</div>
	</div>
</template>

<style scoped>
.project-grid {
	display: grid;
	grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr));
	gap: 0.75rem;
	width: 100%;
}
</style>
