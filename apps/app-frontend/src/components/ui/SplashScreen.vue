<template>
	<Transition name="splash-fade" @after-leave="onAfterLeave">
		<div v-if="!doneLoading" class="splash-screen dark">
			<div class="app-logo-wrapper" data-tauri-drag-region>
				<svg class="app-logo" viewBox="0 0 940 200" fill="none" color="var(--color-contrast)">
					<defs>
						<linearGradient id="splashTile" x1="143" y1="143" x2="369" y2="369" gradientUnits="userSpaceOnUse">
							<stop offset="0" stop-color="#7c5cff" />
							<stop offset="1" stop-color="#4b2fb0" />
						</linearGradient>
					</defs>
					<g transform="translate(4 6) scale(0.363)">
						<path
							d="M256 26 L455 141 L455 371 L256 486 L57 371 L57 141 Z"
							fill="#6c4bff"
							fill-opacity="0.10"
							stroke="#8b6fff"
							stroke-width="22"
							stroke-linejoin="round"
						/>
						<rect x="143" y="143" width="226" height="226" rx="54" fill="url(#splashTile)" />
						<text
							x="256"
							y="260"
							text-anchor="middle"
							dominant-baseline="central"
							font-family="Inter, 'Segoe UI', Arial, sans-serif"
							font-weight="700"
							font-size="172"
							fill="#ffffff"
						>B</text>
					</g>
					<text
						x="214"
						y="133"
						font-family="Inter, 'Segoe UI', Arial, sans-serif"
						font-weight="700"
						font-size="108"
						letter-spacing="-2"
					><tspan fill="currentColor">Byte</tspan><tspan fill="var(--color-brand)">Launcher</tspan></text>
				</svg>
				<ProgressBar class="loading-bar" :progress="Math.min(loadingProgress, 100)" />
				<span v-if="message">{{ message }}</span>
			</div>
			<div class="gradient-bg" data-tauri-drag-region></div>
			<div class="cube-bg"></div>
			<div class="base-bg"></div>
		</div>
	</Transition>
</template>

<script setup>
import { injectLoadingState } from '@modrinth/ui'
import { ref, watch } from 'vue'

import ProgressBar from '@/components/ui/ProgressBar.vue'
import { loading_listener } from '@/helpers/events.js'

const doneLoading = ref(false)
const loadingProgress = ref(0)
const message = ref()

const MIN_DISPLAY_MS = 500
const mountedAt = Date.now()

const loading = injectLoadingState()

function onAfterLeave() {
	loading.setEnabled(true)
}

watch(
	[loading.barEnabled, loading.pending],
	([barEnabled, pending]) => {
		if (barEnabled) {
			return
		}

		if (pending) {
			loadingProgress.value = 0
			fakeLoadingIncrease()
			return
		}

		const elapsed = Date.now() - mountedAt
		const delay = Math.max(0, MIN_DISPLAY_MS - elapsed)

		setTimeout(() => {
			if (loading.pending.value) {
				return
			}
			doneLoading.value = true
		}, delay)
	},
	{ immediate: true },
)

function fakeLoadingIncrease() {
	if (loadingProgress.value < 95) {
		setTimeout(() => {
			loadingProgress.value += 2
			fakeLoadingIncrease()
		}, 5)
	}
}

loading_listener(async (e) => {
	if (e.event.type === 'directory_move') {
		loadingProgress.value = 100 * (e.fraction ?? 1)
		message.value = 'Updating app directory...'
	} else if (e.event.type === 'checking_for_updates') {
		loadingProgress.value = 100 * (e.fraction ?? 1)
		message.value = 'Checking for updates...'
	}
})
</script>

<style scoped lang="scss">
.splash-screen {
	position: fixed;
	inset: 0;
	z-index: 10000;
}

.splash-fade-leave-active {
	transition: opacity 0.3s ease-in-out;
}

.splash-fade-leave-to {
	opacity: 0;
}

.app-logo-wrapper {
	position: absolute;
	height: 100vh;
	width: 100%;

	display: flex;
	flex-direction: column;
	justify-content: center;
	align-items: center;

	gap: 1rem;

	z-index: 9998;
}

.app-logo {
	height: 2.25rem;
	width: fit-content;
}

.loading-bar {
	max-width: 20rem;
}

.gradient-bg {
	position: absolute;
	height: 100vh;
	width: 100vw;
	background:
		linear-gradient(180deg, rgba(108, 75, 255, 0.275) 0%, rgba(22, 20, 43, 0.5) 97.29%),
		linear-gradient(0deg, rgba(22, 24, 28, 0.64), rgba(22, 24, 28, 0.64));
	z-index: 9997;
}

.cube-bg {
	position: absolute;

	left: 50%;
	top: 50%;
	transform: translate(-50%, -50%);

	width: 180vw;
	height: 180vh;
	opacity: 0.8;
	background: #16181c url('@/assets/loading/cube.png') center no-repeat;
	background-size: contain;

	z-index: 9996;
}

.base-bg {
	position: absolute;
	top: 0;
	left: 0;
	width: 100%;
	height: 100%;
	background: var(--color-bg);
	z-index: 9995;
}
</style>
