<script setup lang="ts">
import { computed } from 'vue'

import ClassicGridDisplay from '@/bytelauncher/classic-library/ClassicGridDisplay.vue'
import type { GameInstance } from '@/helpers/types'

type LibraryMode = 'all' | 'modpacks' | 'servers' | 'custom'

const props = defineProps<{
	mode: LibraryMode
	instances: GameInstance[]
	serverProjectIds: Set<string>
}>()

function isServer(instance: GameInstance) {
	const link = instance.link
	if (!link) return false
	if (
		link.type === 'server_project' ||
		link.type === 'server_project_modpack' ||
		link.type === 'modrinth_hosting'
	) {
		return true
	}
	return !!link.project_id && props.serverProjectIds.has(link.project_id)
}

const filteredInstances = computed(() => {
	switch (props.mode) {
		case 'modpacks':
			return props.instances.filter((instance) => instance.link && !isServer(instance))
		case 'servers':
			return props.instances.filter(isServer)
		case 'custom':
			return props.instances.filter((instance) => !instance.link)
		default:
			return props.instances
	}
})
</script>

<template>
	<ClassicGridDisplay :instances="filteredInstances" />
</template>
