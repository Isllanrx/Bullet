param(
    [string]$Version = $env:BULLET_VERSION,
    [switch]$PreRelease = ($env:BULLET_PRERELEASE -eq '1'),
    [switch]$NoLaunch = ($env:BULLET_NO_LAUNCH -eq '1')
)

function Install-Bullet {
    param([string]$Version, [switch]$PreRelease, [switch]$NoLaunch)

    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $repo = 'Isllanrx/Bullet'
    $api = "https://api.github.com/repos/$repo/releases"
    $headers = @{ 'User-Agent' = 'bullet-install'; 'Accept' = 'application/vnd.github+json' }
    $download = @{ 'User-Agent' = 'bullet-install' }

    if (-not [Environment]::Is64BitOperatingSystem) {
        throw 'Bullet needs 64-bit Windows 10 or 11.'
    }

    function Invoke-WithRetry([scriptblock]$Action, [string]$What) {
        for ($attempt = 1; ; $attempt++) {
            try { return & $Action }
            catch {
                if ($attempt -ge 3) { throw "Could not $What after $attempt attempts: $($_.Exception.Message)" }
                Start-Sleep -Seconds (2 * $attempt)
            }
        }
    }

    $release = Invoke-WithRetry -What 'read the release list from GitHub' -Action {
        if ($Version) {
            $tag = if ($Version.StartsWith('v')) { $Version } else { "v$Version" }
            Invoke-RestMethod -Uri "$api/tags/$tag" -Headers $headers -TimeoutSec 30 -UseBasicParsing
        } elseif ($PreRelease) {
            (Invoke-RestMethod -Uri "$api`?per_page=10" -Headers $headers -TimeoutSec 30 -UseBasicParsing) |
                Where-Object { -not $_.draft } | Select-Object -First 1
        } else {
            Invoke-RestMethod -Uri "$api/latest" -Headers $headers -TimeoutSec 30 -UseBasicParsing
        }
    }
    if (-not $release) { throw 'No published Bullet release was found.' }

    $setup = $release.assets | Where-Object { $_.name -match '^Bullet-Setup-.+-x64\.exe$' } | Select-Object -First 1
    $sums = $release.assets | Where-Object { $_.name -eq 'SHA256SUMS' } | Select-Object -First 1
    if (-not $setup -or -not $sums) {
        throw "Release $($release.tag_name) has no installer or SHA256SUMS."
    }
    Write-Host "Bullet $($release.tag_name): downloading $($setup.name)"

    $work = Join-Path ([IO.Path]::GetTempPath()) ("bullet-install-" + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $work | Out-Null
    try {
        $setupPath = Join-Path $work $setup.name
        $sumsPath = Join-Path $work 'SHA256SUMS'
        Invoke-WithRetry -What "download $($setup.name)" -Action {
            Invoke-WebRequest -Uri $setup.browser_download_url -OutFile $setupPath -Headers $download -TimeoutSec 600 -UseBasicParsing
        } | Out-Null
        Invoke-WithRetry -What 'download SHA256SUMS' -Action {
            Invoke-WebRequest -Uri $sums.browser_download_url -OutFile $sumsPath -Headers $download -TimeoutSec 60 -UseBasicParsing
        } | Out-Null

        $expected = Get-Content $sumsPath | ForEach-Object {
            $parts = $_.Trim() -split '\s+\*?', 2
            if ($parts.Count -eq 2 -and $parts[1] -eq $setup.name) { $parts[0].ToLowerInvariant() }
        } | Select-Object -First 1
        if (-not $expected) { throw "SHA256SUMS does not list $($setup.name)." }
        $actual = (Get-FileHash -Path $setupPath -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actual -ne $expected) {
            throw "Checksum mismatch for $($setup.name): expected $expected, got $actual. Nothing was installed."
        }
        Write-Host "Checksum verified: $actual"

        Write-Warning 'Bullet is not code-signed; Windows SmartScreen may warn about the installer.'

        $elevated = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
            [Security.Principal.WindowsBuiltInRole]::Administrator)
        $setupArgs = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- /CLOSEAPPLICATIONS'
        Write-Host 'Installing; Windows asks for administrator permission once, for the copy into Program Files.'
        $process = if ($elevated) {
            Start-Process -FilePath $setupPath -ArgumentList $setupArgs -Wait -PassThru
        } else {
            Start-Process -FilePath $setupPath -ArgumentList $setupArgs -Verb RunAs -Wait -PassThru
        }
        if ($process.ExitCode -ne 0) {
            throw "The installer exited with code $($process.ExitCode)."
        }
    } finally {
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
    }

    $installDir = $null
    try {
        $baseKey = [Microsoft.Win32.RegistryKey]::OpenBaseKey(
            [Microsoft.Win32.RegistryHive]::LocalMachine,
            [Microsoft.Win32.RegistryView]::Registry64
        )
        $subKey = $baseKey.OpenSubKey('SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{D387A5B1-8C56-4D2A-94B8-975DE11C6B45}_is1')
        if ($subKey) {
            $installDir = $subKey.GetValue('InstallLocation')
            $subKey.Close()
        }
        $baseKey.Close()
    } catch {}
    if (-not $installDir) {
        $pf = if ($env:ProgramW6432) { $env:ProgramW6432 } else { $env:ProgramFiles }
        $installDir = Join-Path $pf 'Bullet'
    }
    $exe = Join-Path $installDir 'bullet.exe'
    $launcher = Join-Path $installDir 'tools\bullet_launcher.exe'
    $bridge = Join-Path $installDir 'tools\bullet_bridge.dll'
    if (-not (Test-Path -LiteralPath $exe)) { throw "Bullet was not found at $exe after the install." }
    if (-not (Test-Path -LiteralPath $launcher) -or -not (Test-Path -LiteralPath $bridge)) {
        throw "Bullet bridge tools were not found under $installDir\tools after the install."
    }
    Write-Host "Bullet $($release.tag_name) installed in $installDir"

    if ($NoLaunch) { return }
    Start-Process -FilePath $exe
    Write-Host 'Bullet started in the system tray. Make sure LTK Manager 1.33.0+ is installed.'
}

Install-Bullet -Version $Version -PreRelease:$PreRelease -NoLaunch:$NoLaunch
