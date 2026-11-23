# Installs recdr on Windows:  irm https://recdr.dev/install.ps1 | iex
#
# RECDR_VERSION       install this version (0.1.0 or v0.1.0) instead of the newest
# RECDR_INSTALL_DIR   where recdr.exe goes (default %LOCALAPPDATA%\Programs\recdr)
# RECDR_RELEASE_BASE  where releases are read from (default GitHub releases)
#
# `recdr update` runs this script too, with all three set.

function Install-Recdr {
    # Function scope: under `irm | iex` this does not change the caller's session.
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'
    # Windows PowerShell 5.1 may offer only TLS 1.0, which GitHub refuses.
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $base = if ($env:RECDR_RELEASE_BASE) { $env:RECDR_RELEASE_BASE.TrimEnd('/') } else { 'https://github.com/dzikrisyairozi/recdr/releases' }
    $defaultDir = Join-Path $env:LOCALAPPDATA 'Programs\recdr'
    $dir = if ($env:RECDR_INSTALL_DIR) { $env:RECDR_INSTALL_DIR } else { $defaultDir }

    # Windows on ARM runs the x64 build under emulation.
    $arch = $env:PROCESSOR_ARCHITECTURE
    if ($arch -ne 'AMD64' -and $arch -ne 'ARM64') { throw "recdr install: no recdr build for Windows $arch" }
    $target = 'x86_64-pc-windows-msvc'

    if ($env:RECDR_VERSION) {
        $version = $env:RECDR_VERSION.TrimStart('v')
    } else {
        $url = "$base/latest/download/version.txt"
        try { $text = (Invoke-WebRequest -UseBasicParsing $url).Content } catch { throw "recdr install: could not read $url" }
        if ($text -is [byte[]]) { $text = [Text.Encoding]::ASCII.GetString($text) }
        $version = ($text -split "`n")[0].Trim()
        if (-not $version) { throw "recdr install: $url is empty" }
    }

    $tmp = Join-Path $env:TEMP "recdr-install-$PID"
    New-Item -ItemType Directory -Force $tmp | Out-Null
    try {
        $archive = "recdr-$target.zip"
        $url = "$base/download/v$version/$archive"
        try { Invoke-WebRequest -UseBasicParsing $url -OutFile "$tmp\$archive" } catch { throw "recdr install: could not download $url" }
        try { Invoke-WebRequest -UseBasicParsing "$url.sha256" -OutFile "$tmp\$archive.sha256" } catch { throw "recdr install: could not download $url.sha256" }

        $expected = ((Get-Content -Raw "$tmp\$archive.sha256").Trim() -split '\s+')[0].ToLower()
        $actual = (Get-FileHash -Algorithm SHA256 "$tmp\$archive").Hash.ToLower()
        if ($expected -ne $actual) { throw "recdr install: checksum of $archive does not match; nothing was installed" }

        try { Expand-Archive -Force "$tmp\$archive" "$tmp\unpacked" } catch { throw "recdr install: could not unpack $archive" }
        if (-not (Test-Path "$tmp\unpacked\recdr.exe")) { throw "recdr install: $archive holds no recdr.exe" }

        New-Item -ItemType Directory -Force $dir | Out-Null
        $exe = Join-Path $dir 'recdr.exe'
        $old = "$exe.old"
        Remove-Item $old -Force -ErrorAction SilentlyContinue
        # A running exe can be renamed but not overwritten; the new file takes the name.
        if (Test-Path $exe) { Move-Item -Force $exe $old }
        try { Move-Item "$tmp\unpacked\recdr.exe" $exe }
        catch {
            if (Test-Path $old) { Move-Item -Force $old $exe }
            throw "recdr install: could not write $exe"
        }
        # Still running when this fails; the next install removes it.
        Remove-Item $old -Force -ErrorAction SilentlyContinue
    } finally {
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }

    # Only the default folder is added to the user Path; a chosen folder is the user's to manage.
    if (-not $env:RECDR_INSTALL_DIR) {
        $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        if (-not (($userPath -split ';') -contains $dir)) {
            $newPath = if ($userPath) { "$userPath;$dir" } else { $dir }
            [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
            Write-Host "added $dir to your user Path; open a new terminal to use it"
        }
    }

    Write-Host "installed recdr $version to $exe"
    if (-not (($env:Path -split ';') -contains $dir)) {
        Write-Host "$dir is not on PATH in this terminal"
    }
    if ([Environment]::OSVersion.Version.Build -lt 18362) {
        Write-Host "desktop recording needs Windows 10 1903; tab recording works"
    }
    Write-Host "claude mcp add recdr -- recdr mcp"
}

try { Install-Recdr }
catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    if ($PSCommandPath) { exit 1 }
}
