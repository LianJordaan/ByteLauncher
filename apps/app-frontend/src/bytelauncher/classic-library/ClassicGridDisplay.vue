<script setup lang="ts">
import {
	ClipboardCopyIcon,
	EyeIcon,
	FolderOpenIcon,
	PlayIcon,
	PlusIcon,
	SearchIcon,
	StopCircleIcon,
	TrashIcon,
} from '@modrinth/assets'
import {
	Accordion,
	defineMessages,
	DropdownSelect,
	formatLoader,
	injectNotificationManager,
	StyledInput,
	useVIntl,
} from '@modrinth/ui'
import { useStorage } from '@vueuse/core'
import dayjs from 'dayjs'
import { computed, ref } from 'vue'

import { NewInstanceImage } from '@/assets/icons'
import ContextMenu from '@/components/ui/ContextMenu.vue'
import Instance from '@/components/ui/Instance.vue'
import ConfirmDeleteInstanceModal from '@/components/ui/modal/ConfirmDeleteInstanceModal.vue'
import { useAppEvent } from '@/composables/use-app-event'
import { install_duplicate_instance } from '@/helpers/install'
import { remove } from '@/helpers/instance'
import { type InstanceGroupDefinition, list_groups } from '@/helpers/instance-groups'
import { get_by_instance_id } from '@/helpers/process'
import type { GameInstance } from '@/helpers/types'

const props = defineProps<{ instances: GameInstance[] }>()
const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const messages = defineMessages({
	search: { id: 'bytelauncher.classic-library.search', defaultMessage: 'Search' },
	sortBy: { id: 'bytelauncher.classic-library.sort-by', defaultMessage: 'Sort by:' },
	groupBy: { id: 'bytelauncher.classic-library.group-by', defaultMessage: 'Group by:' },
	play: { id: 'bytelauncher.classic-library.action.play', defaultMessage: 'Play' },
	stop: { id: 'bytelauncher.classic-library.action.stop', defaultMessage: 'Stop' },
	addContent: {
		id: 'bytelauncher.classic-library.action.add-content',
		defaultMessage: 'Add content',
	},
	view: { id: 'bytelauncher.classic-library.action.view', defaultMessage: 'View instance' },
	duplicate: {
		id: 'bytelauncher.classic-library.action.duplicate',
		defaultMessage: 'Duplicate instance',
	},
	delete: { id: 'bytelauncher.classic-library.action.delete', defaultMessage: 'Delete' },
	open: { id: 'bytelauncher.classic-library.action.open', defaultMessage: 'Open folder' },
	copy: { id: 'bytelauncher.classic-library.action.copy', defaultMessage: 'Copy path' },
	empty: { id: 'bytelauncher.classic-library.empty', defaultMessage: 'No instances found' },
})

type InstanceCard = InstanceType<typeof Instance>
type SortBy = 'Name' | 'Last played' | 'Date created' | 'Date modified' | 'Game version'
type GroupBy = 'Group' | 'Loader' | 'Game version' | 'None'
type DisplayState = { group: GroupBy; sortBy: SortBy; collapsedGroups: string[] }

const instanceOptions = ref<InstanceType<typeof ContextMenu>>()
const instanceComponents = ref<InstanceCard[]>([])
const confirmModal = ref<InstanceType<typeof ConfirmDeleteInstanceModal>>()
const currentDeleteInstanceId = ref<string | null>(null)
const groupDefinitions = ref<InstanceGroupDefinition[]>([])
const search = ref('')
const sortOptions: SortBy[] = [
	'Name',
	'Last played',
	'Date created',
	'Date modified',
	'Game version',
]
const groupOptions: GroupBy[] = ['Group', 'Loader', 'Game version', 'None']
const state = useStorage<DisplayState>(
	'Classic-library-grid-display-state',
	{ group: 'Group', sortBy: 'Name', collapsedGroups: [] },
	localStorage,
	{ mergeDefaults: true },
)
const currentDeleteInstances = computed(() =>
	props.instances.filter((instance) => instance.id === currentDeleteInstanceId.value),
)
const collapsedSectionKeys = computed(() => new Set(state.value.collapsedGroups))
const groupNames = computed(
	() => new Map(groupDefinitions.value.map((group) => [group.id, group.name])),
)

async function refreshGroups() {
	groupDefinitions.value = await list_groups().catch((error) => {
		handleError(error)
		return []
	})
}

void refreshGroups()
useAppEvent('instance_groups_changed', refreshGroups)

function sectionKey(sectionName: string) {
	return `${state.value.group}:${sectionName}`
}

function isSectionCollapsed(sectionName: string) {
	return collapsedSectionKeys.value.has(sectionKey(sectionName))
}

function setSectionCollapsed(sectionName: string, collapsed: boolean) {
	const key = sectionKey(sectionName)
	const collapsedSections = new Set(state.value.collapsedGroups)
	if (collapsed) collapsedSections.add(key)
	else collapsedSections.delete(key)
	state.value.collapsedGroups = [...collapsedSections]
}

const groupedInstances = computed(() => {
	const instances = props.instances
		.filter((instance) => instance.name.toLowerCase().includes(search.value.toLowerCase()))
		.slice()

	switch (state.value.sortBy) {
		case 'Name':
			instances.sort((a, b) => a.name.localeCompare(b.name))
			break
		case 'Game version':
			instances.sort((a, b) =>
				a.game_version.localeCompare(b.game_version, undefined, { numeric: true }),
			)
			break
		case 'Last played':
			instances.sort((a, b) => dayjs(b.last_played ?? 0).diff(dayjs(a.last_played ?? 0)))
			break
		case 'Date created':
			instances.sort((a, b) => dayjs(b.created).diff(dayjs(a.created)))
			break
		case 'Date modified':
			instances.sort((a, b) => dayjs(b.modified).diff(dayjs(a.modified)))
	}

	const groups = new Map<string, GameInstance[]>()
	const add = (name: string, instance: GameInstance) => {
		const group = groups.get(name) ?? []
		group.push(instance)
		groups.set(name, group)
	}

	for (const instance of instances) {
		switch (state.value.group) {
			case 'Loader':
				add(formatLoader(formatMessage, instance.loader), instance)
				break
			case 'Game version':
				add(instance.game_version, instance)
				break
			case 'Group':
				if (instance.group_ids.length === 0) add('None', instance)
				else {
					for (const groupId of instance.group_ids)
						add(groupNames.value.get(groupId) ?? groupId, instance)
				}
				break
			default:
				add('None', instance)
		}
	}

	const entries = [...groups.entries()]
	if (state.value.sortBy === 'Name') {
		entries.sort(([a], [b]) => {
			if (a === 'None') return -1
			if (b === 'None') return 1
			return a.localeCompare(b)
		})
	}
	if (state.value.group === 'Game version') {
		entries.sort(([a], [b]) => b.localeCompare(a, undefined, { numeric: true }))
	}
	return entries
})

async function showInstanceMenu(event: MouseEvent, instanceId: string) {
	const item = instanceComponents.value.find((component) => component.instance.id === instanceId)
	if (!item) return
	const runningProcesses = await get_by_instance_id(instanceId).catch((error) => {
		handleError(error)
		return []
	})
	const baseOptions = [
		...(item.instance.quarantined ? [] : [{ name: 'add_content' }, { type: 'divider' }]),
		{ name: 'edit' },
		{ name: 'duplicate' },
		{ name: 'open' },
		{ name: 'copy' },
		{ type: 'divider' },
		{ name: 'delete', color: 'danger' },
	]
	instanceOptions.value?.showMenu(
		event,
		item,
		runningProcesses.length > 0
			? [{ name: 'stop', color: 'danger' }, ...baseOptions]
			: [
					...(item.instance.quarantined ? [] : [{ name: 'play', color: 'primary' }]),
					...baseOptions,
				],
	)
}

async function handleOption({ option, item }: { option: string; item: InstanceCard }) {
	switch (option) {
		case 'play':
			await item.play(null, 'ClassicLibraryContextMenu')
			break
		case 'stop':
			await item.stop(null, 'ClassicLibraryContextMenu')
			break
		case 'add_content':
			await item.addContent()
			break
		case 'edit':
			await item.seeInstance()
			break
		case 'duplicate':
			if (item.instance.install_stage === 'installed') {
				await install_duplicate_instance(item.instance.id).catch(handleError)
			}
			break
		case 'open':
			await item.openFolder()
			break
		case 'copy':
			await navigator.clipboard.writeText(item.instance.path)
			break
		case 'delete':
			currentDeleteInstanceId.value = item.instance.id
			confirmModal.value?.show()
			break
	}
}

async function deleteInstance() {
	if (currentDeleteInstanceId.value) {
		await remove(currentDeleteInstanceId.value).catch(handleError)
		currentDeleteInstanceId.value = null
	}
}
</script>

<template>
	<div class="flex gap-2">
		<StyledInput
			v-model="search"
			:icon="SearchIcon"
			type="text"
			:placeholder="formatMessage(messages.search)"
			clearable
			wrapper-class="flex-1"
		/>
		<DropdownSelect
			v-slot="{ selected }"
			v-model="state.sortBy"
			name="Classic library sort"
			class="max-w-[16rem]"
			:options="sortOptions"
		>
			<span class="font-semibold text-primary">{{ formatMessage(messages.sortBy) }} </span>
			<span class="font-semibold text-secondary">{{ selected }}</span>
		</DropdownSelect>
		<DropdownSelect
			v-slot="{ selected }"
			v-model="state.group"
			name="Classic library group"
			class="max-w-[16rem]"
			:options="groupOptions"
		>
			<span class="font-semibold text-primary">{{ formatMessage(messages.groupBy) }} </span>
			<span class="font-semibold text-secondary">{{ selected }}</span>
		</DropdownSelect>
	</div>
	<Accordion
		v-for="[groupName, groupInstances] in groupedInstances"
		:key="groupName"
		:divider="groupName !== 'None'"
		:open-by-default="!isSectionCollapsed(groupName)"
		class="w-full"
		@on-open="setSectionCollapsed(groupName, false)"
		@on-close="setSectionCollapsed(groupName, true)"
	>
		<template v-if="groupName !== 'None'" #title>
			<span class="text-base">{{ groupName }}</span>
		</template>
		<section class="instance-grid">
			<Instance
				v-for="instance in groupInstances"
				ref="instanceComponents"
				:key="`${groupName}:${instance.id}:${instance.install_stage}`"
				:instance="instance"
				@contextmenu.prevent.stop="(event) => showInstanceMenu(event, instance.id)"
			/>
		</section>
	</Accordion>
	<div v-if="groupedInstances.length === 0" class="flex flex-col items-center gap-3 py-16">
		<NewInstanceImage class="h-40 w-40" />
		<h3 class="m-0">{{ formatMessage(messages.empty) }}</h3>
	</div>
	<ConfirmDeleteInstanceModal
		ref="confirmModal"
		:instances="currentDeleteInstances"
		@delete="deleteInstance"
	/>
	<ContextMenu ref="instanceOptions" @option-clicked="handleOption">
		<template #play><PlayIcon /> {{ formatMessage(messages.play) }}</template>
		<template #stop><StopCircleIcon /> {{ formatMessage(messages.stop) }}</template>
		<template #add_content><PlusIcon /> {{ formatMessage(messages.addContent) }}</template>
		<template #edit><EyeIcon /> {{ formatMessage(messages.view) }}</template>
		<template #duplicate><ClipboardCopyIcon /> {{ formatMessage(messages.duplicate) }}</template>
		<template #delete><TrashIcon /> {{ formatMessage(messages.delete) }}</template>
		<template #open><FolderOpenIcon /> {{ formatMessage(messages.open) }}</template>
		<template #copy><ClipboardCopyIcon /> {{ formatMessage(messages.copy) }}</template>
	</ContextMenu>
</template>

<style scoped>
.instance-grid {
	display: grid;
	grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
	gap: 0.75rem;
	width: 100%;
}
</style>
