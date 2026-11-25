# Installs TotalFloat on Windows (x86-64) from its GitHub release. In PowerShell:
#
#   irm https://raw.githubusercontent.com/ijd5004/totalfloat/main/install/install.ps1 | iex
#
# $env:TOTALFLOAT_VERSION picks a release, such as v0.1.0; the default is the latest.
# It installs to %LOCALAPPDATA%\Programs\totalfloat and adds that folder to the
# user Path. Run it again to upgrade. See docs/guide/install.md.
#
# Runs in Windows PowerShell 5.1 and PowerShell 7. Everything is inside a script
# block, so `iex` leaves no variables or preference changes in the session, and
# errors are thrown instead of calling `exit`, which would close the window.

& {
    $ErrorActionPreference = 'Stop'
    # Invoke-WebRequest's progress bar makes downloads many times slower in 5.1.
    $ProgressPreference = 'SilentlyContinue'

    $releases = 'https://github.com/ijd5004/totalfloat/releases'
    $startHere = 'https://ijd5004.github.io/totalfloat/guide/start-here.html'
    $fromSource = 'https://github.com/ijd5004/totalfloat/blob/main/docs/guide/install.md#build-from-source'

    function Say {
        # The installer talks to the person running it, so it writes to the console.
        [Diagnostics.CodeAnalysis.SuppressMessageAttribute('PSAvoidUsingWriteHost', '')]
        param([string] $Message)
        Write-Host "totalfloat-install: $Message"
    }

    $onWindows = $PSVersionTable.PSEdition -ne 'Core' -or $IsWindows
    if ($onWindows) {
        $os = 'Windows'
        # 32-bit PowerShell on 64-bit Windows reports x86 here and AMD64 in PROCESSOR_ARCHITEW6432.
        $arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
    } else {
        $os = [Runtime.InteropServices.RuntimeInformation]::OSDescription
        $arch = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    }
    if (-not $onWindows -or $arch -ne 'AMD64') {
        throw "There's no prebuilt TotalFloat for $os on $arch. To build it from source, see $fromSource"
    }
    $target = 'x86_64-pc-windows-msvc'

    # Windows PowerShell 5.1 may not offer TLS 1.2, which GitHub requires.
    if ($PSVersionTable.PSEdition -ne 'Core') {
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    }

    $tag = $env:TOTALFLOAT_VERSION
    if (-not $tag) {
        # releases/latest redirects to releases/tag/<tag>.
        $response = Invoke-WebRequest -Uri "$releases/latest" -Method Head -UseBasicParsing
        $final = if ($response.BaseResponse.ResponseUri) {
            $response.BaseResponse.ResponseUri
        } else {
            $response.BaseResponse.RequestMessage.RequestUri
        }
        $tag = ([string] $final).TrimEnd('/').Split('/')[-1]
    }
    if ($tag -match '^[0-9]') { $tag = "v$tag" }
    if ($tag -notmatch '^v[0-9]') { throw "'$tag' isn't a release version, such as v0.1.0" }

    $zip = "totalfloat-$tag-$target.zip"
    $tmp = Join-Path ([IO.Path]::GetTempPath()) ('totalfloat-install-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        Say "downloading $zip"
        Invoke-WebRequest -Uri "$releases/download/$tag/$zip" -OutFile (Join-Path $tmp $zip) -UseBasicParsing
        Invoke-WebRequest -Uri "$releases/download/$tag/SHA256SUMS" -OutFile (Join-Path $tmp 'SHA256SUMS') -UseBasicParsing

        $expected = $null
        foreach ($line in Get-Content (Join-Path $tmp 'SHA256SUMS')) {
            $fields = $line -split '\s+'
            if ($fields.Count -ge 2 -and ($fields[1] -eq $zip -or $fields[1] -eq "*$zip")) {
                $expected = $fields[0]
                break
            }
        }
        if (-not $expected) { throw "SHA256SUMS has no line for $zip. Nothing was installed." }
        $actual = (Get-FileHash (Join-Path $tmp $zip) -Algorithm SHA256).Hash
        if ($actual -ne $expected) {
            throw "$zip doesn't match SHA256SUMS (expected $($expected.ToLower()), got $($actual.ToLower())). Nothing was installed."
        }
        Say 'checksum matches SHA256SUMS'

        $unpacked = Join-Path $tmp 'unpacked'
        Expand-Archive -Path (Join-Path $tmp $zip) -DestinationPath $unpacked
        # v0.1.0 zips hold the files in a top-level folder; later ones hold them at the root.
        $exe = Get-ChildItem -Path $unpacked -Recurse -Filter 'totalfloat.exe' | Select-Object -First 1
        if (-not $exe) { throw "$zip holds no totalfloat.exe. Nothing was installed." }

        $dir = Join-Path $env:LOCALAPPDATA 'Programs\totalfloat'
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
        $installed = Join-Path $dir 'totalfloat.exe'
        Copy-Item -Path $exe.FullName -Destination $installed -Force
        Say "installed $installed"
    } finally {
        Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }

    # Add the folder at the end of the user Path, keeping the folders already
    # there, as docs/guide/install.md does by hand.
    $entries = @([Environment]::GetEnvironmentVariable('Path', 'User') -split ';' | Where-Object { $_ })
    if ($entries -notcontains $dir) {
        [Environment]::SetEnvironmentVariable('Path', ($entries + $dir) -join ';', 'User')
        Say "added $dir to your Path"
    }
    if (@($env:Path -split ';') -notcontains $dir) {
        $env:Path = $env:Path.TrimEnd(';') + ";$dir"
    }

    & $installed --version
    Say "next, open the Start here page: $startHere"
}
