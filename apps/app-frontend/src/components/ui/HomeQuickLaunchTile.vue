<script setup>
import {
	DownloadIcon,
	GameIcon,
	PlayIcon,
	SpinnerIcon,
	StopCircleIcon,
	TimerIcon,
} from '@modrinth/assets'
import { Avatar, IconButton, injectNotificationManager, useRelativeTime } from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import dayjs from 'dayjs'
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import { useAppEvent } from '@/composables/use-app-event'
import { trackEvent } from '@/helpers/analytics'
import { install_existing_instance, install_pack_to_existing_instance } from '@/helpers/install'
import { kill, run } from '@/helpers/instance'
import { get_by_instance_id } from '@/helpers/process'
import { handleSevereError } from '@/store/error.js'

const { handleError } = injectNotificationManager()
const formatRelativeTime = useRelativeTime()
const router = useRouter()

const props = defineProps({
	instance: {
		type: Object,
		required: true,
	},
})

const playing = ref(false)
const loading = ref(false)
const currentEvent = ref(null)

const modLoading = computed(
	() =>
		loading.value ||
		currentEvent.value === 'installing' ||
		(currentEvent.value === 'launched' && !playing.value),
)
const installing = computed(() => props.instance.install_stage.includes('installing'))
const installed = computed(() => props.instance.install_stage === 'installed')

async function checkProcess() {
	const runningProcesses = await get_by_instance_id(props.instance.id).catch(handleError)
	playing.value = runningProcesses.length > 0
}

async function play(e) {
	e?.stopPropagation()
	loading.value = true
	await run(props.instance.id)
		.catch((err) => handleSevereError(err, { instanceId: props.instance.id }))
		.finally(() => {
			trackEvent('InstanceStart', {
				loader: props.instance.loader,
				game_version: props.instance.game_version,
				source: 'HomeDashboard',
			})
		})
	loading.value = false
}

async function stop(e) {
	e?.stopPropagation()
	playing.value = false
	await kill(props.instance.id).catch(handleError)
	trackEvent('InstanceStop', {
		loader: props.instance.loader,
		game_version: props.instance.game_version,
		source: 'HomeDashboard',
	})
}

async function repair(e) {
	e?.stopPropagation()
	if (
		props.instance.install_stage !== 'pack_installed' &&
		(props.instance.link?.type === 'modrinth_modpack' ||
			props.instance.link?.type === 'server_project_modpack')
	) {
		await install_pack_to_existing_instance(props.instance.id, {
			type: 'fromVersionId',
			project_id: props.instance.link.project_id ?? props.instance.link.server_project_id ?? '',
			version_id: props.instance.link.version_id ?? props.instance.link.content_version_id ?? '',
			title: props.instance.name,
		}).catch(handleError)
	} else {
		await install_existing_instance(props.instance.id, false).catch(handleError)
	}
}

function seeInstance() {
	router.push(`/instance/${encodeURIComponent(props.instance.id)}`)
}

useAppEvent('process', (e) => {
	if (e.instance_id === props.instance.id) {
		currentEvent.value = e.event
		if (e.event === 'finished') playing.value = false
	}
})

onMounted(checkProcess)
</script>

<template>
	<div
		class="quick-tile button-base group flex items-center gap-3 rounded-2xl bg-bg-raised p-4"
		@click="seeInstance"
		@mouseenter="checkProcess"
	>
		<Avatar
			size="64px"
			:src="instance.icon_path ? convertFileSrc(instance.icon_path) : null"
			:tint-by="instance.id"
			alt=""
			:class="`shrink-0 transition-all ${modLoading || installing ? 'brightness-[0.35]' : 'group-hover:brightness-90'}`"
		/>
		<div class="flex min-w-0 flex-1 flex-col gap-1">
			<p class="m-0 truncate text-lg font-extrabold leading-tight text-contrast">
				{{ instance.name }}
			</p>
			<div class="flex items-center gap-1 font-semibold text-secondary">
				<GameIcon class="shrink-0" />
				<span class="text-sm capitalize">{{ instance.loader }} {{ instance.game_version }}</span>
			</div>
			<div class="flex items-center gap-1 text-sm text-secondary">
				<TimerIcon class="shrink-0" />
				<span>
					<template v-if="instance.last_played">
						{{ formatRelativeTime(dayjs(instance.last_played).toISOString()) }}
					</template>
					<template v-else>Never played</template>
				</span>
			</div>
		</div>
		<div class="shrink-0" @click.stop>
			<IconButton
				v-if="playing"
				v-tooltip="'Stop'"
				type="colored"
				color="red"
				size="xl"
				label="Stop"
				@click="stop"
			>
				<StopCircleIcon />
			</IconButton>
			<IconButton
				v-else-if="modLoading || installing"
				v-tooltip="'Loading…'"
				size="xl"
				label="Loading"
				disabled
			>
				<SpinnerIcon class="animate-spin" />
			</IconButton>
			<IconButton
				v-else-if="!installed"
				v-tooltip="'Repair'"
				type="colored"
				color="brand"
				size="xl"
				label="Repair"
				@click="repair"
			>
				<DownloadIcon />
			</IconButton>
			<IconButton
				v-else
				v-tooltip="'Play'"
				type="colored"
				color="brand"
				size="xl"
				label="Play"
				@click="play"
			>
				<PlayIcon class="translate-x-[2px]" />
			</IconButton>
		</div>
	</div>
</template>
