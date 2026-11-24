$ErrorActionPreference = 'Stop'
$stage = $env:OXIGRAM_UPDATE_STAGE
$root = $env:OXIGRAM_UPDATE_ROOT
$incoming = $env:OXIGRAM_UPDATE_INCOMING
$backup = Join-Path $stage 'previous'
$marker = Join-Path $root $env:OXIGRAM_UPDATE_MARKER
$exe = Join-Path $root $env:OXIGRAM_UPDATE_EXE
$launcher = $env:OXIGRAM_UPDATE_LAUNCHER_PID
$workingDirectory = $env:OXIGRAM_UPDATE_WORKDIR
if ($launcher) {
    $exe = $root
}
if (-not $workingDirectory -or -not (Test-Path -LiteralPath $workingDirectory -PathType Container)) {
    $workingDirectory = Split-Path -LiteralPath $exe
}

try {
    $parent = Get-Process -Id ([int]$env:OXIGRAM_UPDATE_PID) -ErrorAction SilentlyContinue
    if ($null -ne $parent) {
        Wait-Process -InputObject $parent -Timeout 30 -ErrorAction Stop
    }
    if ($launcher) {
        $parent = Get-Process -Id ([int]$launcher) -ErrorAction SilentlyContinue
        if ($null -ne $parent) {
            Wait-Process -InputObject $parent -Timeout 30 -ErrorAction Stop
        }
        $existing = Get-Item -LiteralPath $root
        if ($existing.PSIsContainer -or ($existing.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
            (Get-FileHash -LiteralPath $root -Algorithm SHA256).Hash -ne $env:OXIGRAM_UPDATE_FINGERPRINT) {
            return
        }
    } else {
        $lines = @(Get-Content -LiteralPath $marker)
        if ($lines.Count -lt 3 -or $lines[1] -ne $env:OXIGRAM_UPDATE_FROM) {
            return
        }
        $expected = @($lines | Select-Object -Skip 2)
        $prefix = $root.Length + 1
        $actual = @(Get-ChildItem -LiteralPath $root -Force -Recurse | ForEach-Object {
            './' + $_.FullName.Substring($prefix).Replace('\', '/')
        })
        if (@(Compare-Object -ReferenceObject $expected -DifferenceObject $actual).Count -ne 0) {
            Start-Process -FilePath $exe -WorkingDirectory $workingDirectory
            return
        }
    }

    function Move-Package($From, $To) {
        if ($launcher) {
            [IO.File]::Move($From, $To)
        } else {
            [IO.Directory]::Move($From, $To)
        }
    }
    try {
        Move-Package $root $backup
    } catch {
        Start-Process -FilePath $exe -WorkingDirectory $workingDirectory
        throw
    }
    try {
        Move-Package $incoming $root
        if ($launcher) {
            $ready = Join-Path $stage 'ready'
            $env:OXIGRAM_UPDATE_READY = $ready
        }
        try {
            $running = Start-Process -FilePath $exe -WorkingDirectory $workingDirectory -PassThru
        } finally {
            Remove-Item Env:OXIGRAM_UPDATE_READY -ErrorAction SilentlyContinue
        }
        if ($launcher) {
            for ($attempt = 0; -not (Test-Path -LiteralPath $ready); $attempt++) {
                if ($running.WaitForExit(200)) { throw 'OxiGram exited before reporting startup.' }
                if ($attempt -ge 149) { return } # Keep the backup if startup is still in progress.
            }
        } elseif ($running.WaitForExit(500)) {
            throw 'OxiGram exited immediately after updating.'
        }
    } catch {
        if (Test-Path -LiteralPath $root) {
            Move-Package $root (Join-Path $stage 'failed')
        }
        Move-Package $backup $root
        Start-Process -FilePath $exe -WorkingDirectory $workingDirectory
        throw
    }
    Remove-Item -Recurse -Force -LiteralPath $backup
} finally {
    if (-not (Test-Path -LiteralPath $backup)) {
        Remove-Item -Recurse -Force -LiteralPath $stage
    }
}
