# Installs Allot on Windows from a GitHub release: downloads the setup, checks it against its
# .sha256, and runs it silently. The setup runs `allot.exe install`, which does the real setup,
# and adds the Apps & features entry for uninstalling.
#
#   irm https://raw.githubusercontent.com/meh7an/allot/main/install.ps1 | iex
#   $script = irm https://raw.githubusercontent.com/meh7an/allot/main/install.ps1
#   & ([scriptblock]::Create($script)) -Version 0.3.0 -NoClaude
#
# -Version (or ALLOT_VERSION) installs that release instead of the latest. -NoClaude leaves
# Claude Code alone, and -NoLaunchAtLogon keeps the tray app from starting at sign-in; a later
# run without them keeps the choices of the last install. ALLOT_DOWNLOAD_BASE replaces
# https://github.com/meh7an/allot/releases, for a mirror or a test.
#
# Everything runs in one function, called at the end, so a download cut off halfway runs
# nothing, and its settings stay out of the session that ran `iex`.

param(
    [string]$Version = $env:ALLOT_VERSION,
    [switch]$NoClaude,
    [switch]$NoLaunchAtLogon
)

function Install-Allot {
    param([string]$Version, [bool]$NoClaude, [bool]$NoLaunchAtLogon)

    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'
    # The first release whose setup is named allot-<version>-setup.exe.
    $firstVersion = [version]'0.3.0'
    # The oldest build the setup runs on (its MinVersion), Windows 10 1809.
    $minBuild = 17763

    function Fail([string]$Message) {
        throw "allot-install: error: $Message"
    }

    function Say([string]$Message) {
        Write-Host "allot-install: $Message"
    }

    if ($PSVersionTable.PSEdition -eq 'Core' -and -not $IsWindows) {
        Fail ('on Linux and macOS, run install.sh instead: curl -fsSL ' +
            'https://raw.githubusercontent.com/meh7an/allot/main/install.sh | sh')
    }
    # A 32-bit PowerShell on 64-bit Windows reports x86 here, and the real one in W6432.
    $arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else {
        $env:PROCESSOR_ARCHITECTURE
    }
    if ($arch -ne 'AMD64') {
        Fail "Windows on $arch is not supported: the setup is built for x64"
    }
    $build = [Environment]::OSVersion.Version.Build
    if ($build -lt $minBuild) {
        Fail "Allot needs Windows 10 1809 (build $minBuild) or newer, and this is build $build"
    }

    # Windows PowerShell 5.1 may not offer TLS 1.2, which GitHub requires, by default.
    [Net.ServicePointManager]::SecurityProtocol =
        [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $base = if ($env:ALLOT_DOWNLOAD_BASE) { $env:ALLOT_DOWNLOAD_BASE.TrimEnd('/') } else {
        'https://github.com/meh7an/allot/releases'
    }
    if (-not $Version) {
        try {
            $response = Invoke-WebRequest -Uri "$base/latest" -UseBasicParsing
        } catch {
            Fail "could not reach $base/latest ($($_.Exception.Message))"
        }
        # Where releases/latest redirected to: ResponseUri in 5.1, RequestMessage in 7.
        $final = if ($response.BaseResponse.ResponseUri) {
            $response.BaseResponse.ResponseUri.AbsoluteUri
        } else {
            $response.BaseResponse.RequestMessage.RequestUri.AbsoluteUri
        }
        $tag = $final.Split('/')[-1]
        if ($tag -notmatch '^v\d') { Fail "could not tell the latest release from $final" }
        $Version = $tag
    }
    $Version = $Version.TrimStart('v')
    if ($Version -notmatch '^\d+\.\d+\.\d+$') {
        Fail "'$Version' is not a release version such as $firstVersion"
    }
    if ([version]$Version -lt $firstVersion) {
        Fail "Allot $Version has no setup this script can run; the first is $firstVersion"
    }

    $setup = "allot-$Version-setup.exe"
    $work = Join-Path ([IO.Path]::GetTempPath()) "allot-install-$([guid]::NewGuid())"
    New-Item -ItemType Directory -Path $work | Out-Null
    try {
        Say "downloading Allot $Version for Windows x64"
        foreach ($file in $setup, "$setup.sha256") {
            $url = "$base/download/v$Version/$file"
            try {
                Invoke-WebRequest -Uri $url -OutFile (Join-Path $work $file) -UseBasicParsing
            } catch {
                Fail "could not download $url ($($_.Exception.Message))"
            }
        }

        $listed, $name = (Get-Content -Raw (Join-Path $work "$setup.sha256")).Trim() -split '\s+'
        if (([string]$name).TrimStart('*') -ne $setup) {
            Fail "$setup.sha256 does not hold a checksum for $setup"
        }
        $actual = (Get-FileHash (Join-Path $work $setup) -Algorithm SHA256).Hash.ToLower()
        if ($actual -ne $listed.ToLower()) {
            Fail ("checksum mismatch for ${setup}: expected $listed, got $actual. " +
                'Nothing was installed.')
        }
        Say "checked $setup against its .sha256"

        $off = @()
        if ($NoClaude) { $off += '!claude' }
        if ($NoLaunchAtLogon) { $off += '!launchatlogon' }
        $arguments = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP-'
        if ($off) { $arguments += " /MERGETASKS=$($off -join ',')" }
        Say 'running the setup'
        $process = Start-Process -FilePath (Join-Path $work $setup) -ArgumentList $arguments `
            -Wait -PassThru
        if ($process.ExitCode -ne 0) { Fail "the setup exited with $($process.ExitCode)" }
    } finally {
        Remove-Item -Recurse -Force -Path $work -ErrorAction SilentlyContinue
    }

    # The setup reports success either way; what allot.exe install did is in its log.
    $programs = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Programs'
    $log = Join-Path $programs 'Allot\install.log'
    if (-not (Test-Path $log)) { Fail "the setup left no log at $log" }
    $lines = Get-Content $log
    $lines | ForEach-Object { Write-Host $_ }
    if ($lines | Where-Object { $_ -like 'Error:*' }) {
        Fail "allot.exe install failed; the log above is in $log"
    }
    $bin = Join-Path $env:USERPROFILE '.allot\bin'
    $allot = Join-Path $bin 'allot.exe'
    if (-not (Test-Path $allot)) { Fail "the setup did not put allot.exe in $bin" }
    $installed = & $allot --version
    if ($installed -ne "allot $Version") {
        Fail "expected allot $Version in $bin, found '$installed'"
    }

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (-not ($userPath -split ';' | Where-Object { $_.TrimEnd('\') -eq $bin })) {
        Say "to type allot anywhere, add $bin to your user PATH:"
        Say ("  [Environment]::SetEnvironmentVariable('Path', `"$bin;`$([Environment]::" +
            "GetEnvironmentVariable('Path', 'User'))`", 'User')")
    }
    Say "Allot $Version is installed. Start a new Claude Code session to use it."
}

Install-Allot -Version $Version -NoClaude ([bool]$NoClaude) `
    -NoLaunchAtLogon ([bool]$NoLaunchAtLogon)
