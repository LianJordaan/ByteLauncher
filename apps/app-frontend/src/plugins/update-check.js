import { getVersion } from '@tauri-apps/api/app'
import { openUrl } from '@tauri-apps/plugin-opener'

// ByteLauncher: checks the fork's latest GitHub release on startup and shows
// a dismissible banner when a newer version is available. This is the update
// path for builds that ship without the bundled Tauri updater. It is fully
// self-contained (plain DOM) so it cannot interfere with Vue rendering.

const REPO = 'LianJordaan/ByteLauncher'
const BANNER_ID = 'mr-upstream-update-banner'
const DISMISS_KEY = 'mr-dismissed-update-version'

function showBanner(version, url) {
	if (document.getElementById(BANNER_ID)) return
	if (localStorage.getItem(DISMISS_KEY) === version) return

	const bar = document.createElement('div')
	bar.id = BANNER_ID
	bar.style.cssText = [
		'position:fixed',
		'left:0',
		'right:0',
		'bottom:0',
		'z-index:9998',
		'display:flex',
		'align-items:center',
		'justify-content:center',
		'gap:12px',
		'padding:8px 16px',
		'font-size:13px',
		'background:var(--color-raised-bg, #27292e)',
		'color:var(--color-contrast, #fff)',
		'border-top:1px solid var(--color-divider, #34363c)',
	].join(';')

	const text = document.createElement('span')
	text.textContent = `ByteLauncher ${version} is available.`

	const view = document.createElement('button')
	view.textContent = 'View release'
	view.style.cssText = [
		'padding:4px 12px',
		'border:none',
		'border-radius:6px',
		'cursor:pointer',
		'font-weight:600',
		'background:var(--color-brand, #6c4bff)',
		'color:#fff',
	].join(';')
	view.addEventListener('click', () => {
		openUrl(url).catch((e) => console.error('[update-check] failed to open release', e))
	})

	const dismiss = document.createElement('button')
	dismiss.textContent = '✕'
	dismiss.setAttribute('aria-label', 'Dismiss')
	dismiss.style.cssText = [
		'padding:4px 8px',
		'border:none',
		'border-radius:6px',
		'cursor:pointer',
		'background:transparent',
		'color:inherit',
		'font-size:14px',
	].join(';')
	dismiss.addEventListener('click', () => {
		localStorage.setItem(DISMISS_KEY, version)
		bar.remove()
	})

	bar.append(text, view, dismiss)
	document.body.appendChild(bar)
}

export async function checkForUpdates() {
	try {
		const current = await getVersion()
		// Skip local/dev/canary builds where the version is not a real release.
		if (/local|dev|canary/i.test(current)) return

		const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
			headers: { Accept: 'application/vnd.github+json' },
		})
		if (!res.ok) return

		const data = await res.json()
		const latest = String(data.tag_name || '').replace(/^v/, '')
		if (latest && latest !== current) {
			showBanner(latest, data.html_url || `https://github.com/${REPO}/releases/latest`)
		}
	} catch {
		// Offline or rate-limited — silently skip.
	}
}
