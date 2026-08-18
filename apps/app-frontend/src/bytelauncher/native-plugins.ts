import type { RouteRecordRaw } from 'vue-router'

import ClassicLibrary from '@/bytelauncher/classic-library/ClassicLibrary.vue'
import ClassicLibraryTab from '@/bytelauncher/classic-library/ClassicLibraryTab.vue'
import HomeRoute from '@/bytelauncher/classic-library/HomeRoute.vue'

export const CLASSIC_LIBRARY_PLUGIN_ID = 'classic-library'

export const nativePluginRoutes: RouteRecordRaw[] = [
	{
		path: '/library',
		name: 'ClassicLibrary',
		component: ClassicLibrary,
		children: [
			{
				path: '',
				name: 'ClassicLibraryAll',
				component: ClassicLibraryTab,
				props: { mode: 'all' },
			},
			{
				path: 'modpacks',
				name: 'ClassicLibraryModpacks',
				component: ClassicLibraryTab,
				props: { mode: 'modpacks' },
			},
			{
				path: 'servers',
				name: 'ClassicLibraryServers',
				component: ClassicLibraryTab,
				props: { mode: 'servers' },
			},
			{
				path: 'custom',
				name: 'ClassicLibraryCustom',
				component: ClassicLibraryTab,
				props: { mode: 'custom' },
			},
		],
	},
]

export { HomeRoute }
