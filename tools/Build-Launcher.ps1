# Build the launcher and give the Windows executable its product filename.
# Only the launcher is built; probes are opt-in. Cargo cannot name a binary
# with a space, so the `KH2RW` artifact is renamed to `KH2RW Launcher.exe`. The
# game and the model/image viewers run inside the launcher executable (it
# restarts itself with a program name). Other executables that earlier Cargo
# builds left in the output folder are removed; `cargo build`/`cargo run`
# writes them again from its cache when asked for them.
param([switch]$Debug, [switch]$Skate3, [switch]$Timings)

$ErrorActionPreference = 'Stop'
$env:PATH = (Join-Path $env:USERPROFILE '.cargo/bin') + [IO.Path]::PathSeparator + $env:PATH
$launcherProfile = if ($Debug) { 'dev' } else { 'release' }
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    $launcherTargets = @('--bin', 'KH2RW')
    if ($Skate3) {
        $launcherTargets += @('--features', 'skate3')
    }
    if ($Timings) {
        $launcherTargets += '--timings'
    }
    $launcherBuildOutput = & cargo build -p main @launcherTargets --profile $launcherProfile --locked --message-format=json-render-diagnostics
    if ($LASTEXITCODE -ne 0) { throw 'Launcher build failed.' }
    $launcherArtifact = $launcherBuildOutput | ForEach-Object { $_ | ConvertFrom-Json } |
        Where-Object { $_.reason -eq 'compiler-artifact' -and $_.target.name -eq 'KH2RW' -and $_.executable } |
        Select-Object -Last 1
    if (-not $launcherArtifact) { throw 'Cargo did not report a launcher executable.' }
    $outputDirectory = Split-Path $launcherArtifact.executable -Parent
    $launcherOutputPath = Join-Path $outputDirectory 'KH2RW Launcher.exe'
    Move-Item -LiteralPath $launcherArtifact.executable -Destination $launcherOutputPath -Force
    Get-ChildItem -LiteralPath $outputDirectory -Filter '*.exe' -File |
        Where-Object { $_.Name -ne 'KH2RW Launcher.exe' } |
        ForEach-Object {
            try {
                foreach ($companion in '.pdb', '.d') {
                    $path = Join-Path $outputDirectory ($_.BaseName + $companion)
                    if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
                }
                Remove-Item -LiteralPath $_.FullName -Force
            } catch {
                Write-Warning "Could not remove $($_.TargetObject): $($_.Exception.Message)"
            }
        }
    Write-Output $launcherOutputPath
} finally {
    Pop-Location
}
