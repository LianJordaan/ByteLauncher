param([Parameter(Mandatory = $true)][string]$OutFile)

# Downloads the latest ByteLauncher.exe from GitHub Releases using the stable
# non-API "releases/latest/download" redirect. This is served from GitHub's
# release CDN and is NOT subject to the api.github.com rate limit (60/hr per IP),
# which the old API-based approach hit. Verified against a published SHA-256
# sidecar when present (best-effort: proceeds on the HTTPS download otherwise).

$ErrorActionPreference = 'Stop'
# Windows PowerShell 5.1 may not negotiate TLS 1.2 by default, which GitHub requires.
try { [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12 } catch {}

$base = 'https://github.com/LianJordaan/ByteLauncher/releases/latest/download'

Invoke-WebRequest -UseBasicParsing -Uri "$base/ByteLauncher.exe" -OutFile $OutFile

$expected = $null
try {
    $shaFile = "$OutFile.sha256"
    Invoke-WebRequest -UseBasicParsing -Uri "$base/ByteLauncher.exe.sha256" -OutFile $shaFile
    $expected = ((Get-Content -Raw -Path $shaFile) -split '\s+')[0].Trim().ToLower()
} catch {
    $expected = $null
}

if ($expected) {
    $actual = (Get-FileHash -Algorithm SHA256 -Path $OutFile).Hash.ToLower()
    if ($actual -ne $expected) {
        Remove-Item -Force $OutFile
        Write-Host 'Downloaded file failed SHA-256 verification'
        exit 4
    }
}

exit 0
