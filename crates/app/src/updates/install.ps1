$ErrorActionPreference = 'Stop'
$stage = $env:OXIGRAM_UPDATE_STAGE
$root = $env:OXIGRAM_UPDATE_ROOT
$incoming = $env:OXIGRAM_UPDATE_INCOMING
$backup = Join-Path $stage 'previous'
$marker = Join-Path $root $env:OXIGRAM_UPDATE_MARKER
$exe = Join-Path $root $env:OXIGRAM_UPDATE_EXE
$workingDirectory = $env:OXIGRAM_UPDATE_WORKDIR
if (-not $workingDirectory -or -not (Test-Path -LiteralPath $workingDirectory -PathType Container)) {
    $workingDirectory = $root
}

try {
    $parent = Get-Process -Id ([int]$env:OXIGRAM_UPDATE_PID) -ErrorAction SilentlyContinue
    if ($null -ne $parent) {
        Wait-Process -InputObject $parent -Timeout 30 -ErrorAction Stop
    }
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
    try {
        [System.IO.Directory]::Move($root, $backup)
    } catch {
        Start-Process -FilePath $exe -WorkingDirectory $workingDirectory
        throw
    }
    try {
        [System.IO.Directory]::Move($incoming, $root)
        $running = Start-Process -FilePath $exe -WorkingDirectory $workingDirectory -PassThru
        if ($running.WaitForExit(500)) { throw 'OxiGram exited immediately after updating.' }
    } catch {
        if (Test-Path -LiteralPath $root) {
            [System.IO.Directory]::Move($root, (Join-Path $stage 'failed'))
        }
        [System.IO.Directory]::Move($backup, $root)
        Start-Process -FilePath $exe -WorkingDirectory $workingDirectory
        throw
    }
    Remove-Item -Recurse -Force -LiteralPath $backup
} finally {
    if (-not (Test-Path -LiteralPath $backup)) {
        Remove-Item -Recurse -Force -LiteralPath $stage
    }
}
