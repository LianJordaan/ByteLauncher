import { buildLocaleMessages, createMessageCompiler, type CrowdinMessages } from '@modrinth/ui'
import { uiLocaleModulesEager } from '@modrinth/ui/src/locales.eager.ts'
import { createI18n } from 'vue-i18n'

const localeModules = import.meta.glob<{ default: CrowdinMessages }>('./locales/*/index.json', {
	eager: true,
})

function applyByteLauncherBranding(value: unknown): unknown {
	if (typeof value === 'string') {
		return value.replaceAll('Modrinth App', 'ByteLauncher')
	}
	if (Array.isArray(value)) {
		return value.map(applyByteLauncherBranding)
	}
	if (value && typeof value === 'object') {
		for (const [key, child] of Object.entries(value)) {
			;(value as Record<string, unknown>)[key] = applyByteLauncherBranding(child)
		}
	}
	return value
}

const messages = applyByteLauncherBranding(
	buildLocaleMessages(localeModules, uiLocaleModulesEager),
) as ReturnType<typeof buildLocaleMessages>

const i18n = createI18n({
	legacy: false,
	locale: 'en-US',
	fallbackLocale: 'en-US',
	messageCompiler: createMessageCompiler(),
	missingWarn: false,
	fallbackWarn: false,
	messages,
})

export default i18n
