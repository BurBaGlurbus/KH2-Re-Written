# Source/process-bound Steam research admission. Never executes captured code.
param([Parameter(Mandatory)][string]$ToolDirectory,[Parameter(Mandatory)][string]$JavaHome,
    [Parameter(Mandatory)][string]$InstallRoot,[Parameter(Mandatory)][string]$SourceManifest,
    [Parameter(Mandatory)][string]$CaptureDirectory,[Parameter(Mandatory)][string]$ConformanceReceipt,
    [Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'NativeRangeEngine.ps1')
. (Join-Path $PSScriptRoot 'NativeReader.ps1')
$source=Get-ResearchCanonicalPath $InstallRoot
$out=Assert-PrivateResearchDirectory $OutputDirectory $source
$captured=Assert-PrivateResearchDirectory $CaptureDirectory $source
if (@(Get-ChildItem $out -Force).Count) {throw 'Fresh empty source-bound memory output required'}
function Read-PrivatePrerequisite([string]$Path) {
    $canonical=Get-ResearchCanonicalPath $Path
    [void](Assert-PrivateResearchDirectory (Split-Path $canonical -Parent) $source)
    if ((Get-Item $canonical).Length -gt 1048576) {throw 'Private prerequisite exceeds 1 MiB'}
    # Preserve exact receipt timestamps; implicit DateTime coercion changes comparison text.
    return Get-Content $canonical -Raw|ConvertFrom-Json -DateKind String
}
$manifest=Read-PrivatePrerequisite $SourceManifest
$capture=Read-PrivatePrerequisite ($captured+'/parent-live-ranges.json')
$runtime=Read-PrivatePrerequisite ($captured+'/runtime-module-inventory.json')
$oracle=Read-PrivatePrerequisite $ConformanceReceipt
$engine=Get-NativeRangeEngineIdentity $ToolDirectory $JavaHome
foreach ($field in @('package_sha256','java_sha256','script_sha256','engine_helper_sha256','loader_profile')) {
    if ($oracle.engine_identity.$field -cne $engine.$field) {throw 'Memory oracle engine/host/profile identity differs'}
}
if ($oracle.verdict -cne 'PASS' -or $oracle.loader_profile -cne $engine.loader_profile -or
    $oracle.cases.Count -ne 22 -or $oracle.controls.Count -ne 3 -or $oracle.engine_runs.Count -ne 4 -or
    $oracle.fixture_source_sha256 -cne (Get-FileHash (Join-Path $PSScriptRoot 'fixtures/NativeConformance.c')).Hash -or
    @($oracle.engine_identity.core_components.psobject.Properties).Count -ne 8) {throw 'Memory oracle qualification differs'}
foreach ($name in $engine.core_components.Keys) {
    if ($oracle.engine_identity.core_components.$name -cne $engine.core_components[$name]) {throw 'Memory oracle core component differs'}
}
$inputs=[uint32[]]@(0,7,15,16,255,256,1024,2147483647,2147483648,4294967294,4294967295)
foreach ($relocation in @(0,1)) {
    foreach ($value in $inputs) {
        $case=@($oracle.cases|Where-Object {$_.relocation -eq $relocation -and $_.input -eq $value})
        $expected=if ($value -lt 16) {($value -bxor 90)*3} else {($value -band 255)+7}
        if ($case.Count -ne 1 -or $case[0].expected -ne $expected -or $case[0].original -ne $expected -or
            $case[0].recovered -ne $expected) {throw 'Memory oracle finite behavior differs'}
    }
}
foreach ($name in @('rip-read','outside-call','truncated-entry')) {
    $checks=@($oracle.controls|Where-Object name -CEQ $name)
    $expected=if ($name -eq 'truncated-entry') {'PASS rejection'} else {'PASS'}
    if ($checks.Count -ne 1 -or $checks[0].verdict -cne $expected) {throw 'Memory oracle reference/rejection control differs'}
}
if ($capture.verdict -cne 'PASS bounded observation' -or $capture.process_memory_modified -ne $false -or
    $capture.custom_code_memory_read -ne $true -or $capture.atomic_snapshot -ne $false -or
    $capture.loaded_image_identity_verified -ne $false -or $capture.complete_logical_function_bounds_verified -ne $false -or
    $capture.record_reader_sha256 -cne (Get-FileHash (Join-Path $PSScriptRoot 'NativeReader.ps1')).Hash -or
    $capture.helper_sha256 -cne (Get-FileHash (Join-Path $PSScriptRoot 'ReadOnlyRangeProbe.cs')).Hash -or
    $capture.ranges.Count -lt 1 -or $capture.ranges.Count -gt 8 -or !$capture.directory_repeats_exact) {
    throw 'Memory capture scope/reader/budget differs'
}
$path=Get-ResearchCanonicalPath (Join-Path $source 'KINGDOM HEARTS HD 1.5+2.5 ReMIX.exe')
if ((Get-Item $path).Length -gt 1048576) {throw 'Shared parent exceeds qualified file bound'}
$raw=[IO.File]::ReadAllBytes($path)
$hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($raw)).ToLowerInvariant()
if ($hash -cne $capture.source_sha256 -or $manifest.content_identity -cne $capture.source_content_identity) {throw 'Capture/source identity differs'}
[void](Assert-NativeSharedModuleEvidence $manifest $runtime $source 'KINGDOM HEARTS HD 1.5+2.5 ReMIX.exe' $raw.LongLength $hash $capture.source_content_identity)
$memory=[IO.MemoryStream]::new($raw,$false);$pe=[Reflection.PortableExecutable.PEReader]::new($memory)
try {
    $headers=$pe.PEHeaders
    if ($headers.CoffHeader.Machine -ne [Reflection.PortableExecutable.Machine]::Amd64 -or
        $headers.PEHeader.Magic -ne [Reflection.PortableExecutable.PEMagic]::PE32Plus -or $headers.CorHeader -or
        $headers.PEHeader.SizeOfImage -ne $capture.module_memory_size) {throw 'Capture native image shape differs'}
    $dir=$headers.PEHeader.ExceptionTableDirectory
    $records=@(Get-NativeRuntimeFunctionRecords $raw $headers.SectionHeaders $dir $headers.PEHeader.SizeOfImage)
    if ($records.Count -ne $capture.records -or $dir.RelativeVirtualAddress -ne $capture.directory_rva -or
        $dir.Size -ne $capture.directory_size -or $dir.Size -gt 4096) {throw 'Capture exception table shape differs'}
    $offset=Get-NativePeOffset $headers.SectionHeaders $dir.RelativeVirtualAddress 12 $raw.LongLength
    $last=Get-NativePeOffset $headers.SectionHeaders ([long]$dir.RelativeVirtualAddress+$dir.Size-12) 12 $raw.LongLength
    if ($last-$offset -ne $dir.Size-12) {throw 'Capture table is not contiguous'}
    $table=[byte[]]$raw[$offset..($offset+$dir.Size-1)]
    foreach ($pass in @(0,1)) {
        $file=Join-Path $captured "parent-exception-table-$pass.bin"
        if ((Get-Item $file).Length -ne $dir.Size -or
            [Convert]::ToHexString([IO.File]::ReadAllBytes($file)) -cne [Convert]::ToHexString($table)) {throw 'Complete captured/source table differs'}
    }
    $seen=[Collections.Generic.HashSet[long]]::new();$snapshots=[Collections.Generic.List[object]]::new();$index=0
    foreach ($row in $capture.ranges) {
        $matches=@($records|Where-Object {$_.begin_rva -eq $row.rva -and $_.end_rva-$_.begin_rva -eq $row.size})
        $export=Get-NativeNamedExport $raw $headers.SectionHeaders $headers.PEHeader.ExportTableDirectory $row.name
        if (!$seen.Add($row.rva) -or $row.size -lt 1 -or $row.size -gt 4096 -or $matches.Count -ne 1 -or
            $export.rva -ne $row.rva -or $matches[0].code_file_offset -ne $row.source_file_offset -or !$row.memory_repeats_exact) {
            throw 'Selected captured export/range differs'
        }
        $before=Join-Path $captured "parent-code-range-$index-before.bin";$after=Join-Path $captured "parent-code-range-$index-after.bin"
        foreach ($file in @($before,$after)) {
            if ((Get-Item $file).Length -ne $row.size -or (Get-FileHash $file).Hash.ToLowerInvariant() -cne $row.before_sha256) {
                throw 'Selected capture bytes differ'
            }
        }
        $bytes=[IO.File]::ReadAllBytes($before)
        if ([Convert]::ToHexString($bytes) -cne [Convert]::ToHexString([IO.File]::ReadAllBytes($after))) {throw 'Complete captured repeats differ'}
        $snapshots.Add([pscustomobject]@{row=$row;file=$before;bytes=$bytes;index=$index});$index++
    }
} finally {$pe.Dispose();$memory.Dispose()}
function Get-BoundCurrentParent {
    $proc=Get-Process -Id $capture.process_id -ErrorAction Stop
    if ($proc.HasExited -or $proc.StartTime.ToUniversalTime().ToString('o') -cne $capture.process_start_utc -or
        (Get-ResearchCanonicalPath $proc.MainModule.FileName) -cne $path -or
        $proc.MainModule.BaseAddress.ToInt64() -ne $capture.module_base -or $proc.MainModule.ModuleMemorySize -ne $capture.module_memory_size -or
        (Get-FileHash $path).Hash.ToLowerInvariant() -cne $hash) {throw 'Current source/process/module binding differs'}
    return $proc
}
$proc=Get-BoundCurrentParent
$scanner=Join-Path (Split-Path $PSScriptRoot -Parent) 'target/debug/validation.exe'
# The launcher build scripts remove other executables from target/debug.
& cargo build -p validation --bin validation --locked --manifest-path (Join-Path (Split-Path $PSScriptRoot -Parent) 'Cargo.toml')
if ($LASTEXITCODE) {throw 'validation build failed'}
& $scanner $source ($out+'/source-session.json') *> ($out+'/source-session.log')
if ($LASTEXITCODE) {throw 'Fresh full selected-source scan failed'}
$fresh=Read-PrivatePrerequisite ($out+'/source-session.json')
if (($manifest|ConvertTo-Json -Depth 8 -Compress) -cne ($fresh|ConvertTo-Json -Depth 8 -Compress)) {
    throw 'Fresh full selected-source manifest differs'
}
$proc=Get-BoundCurrentParent
Add-Type -TypeDefinition ([IO.File]::ReadAllText((Join-Path $PSScriptRoot 'ReadOnlyRangeProbe.cs')))
function Read-BoundRange($Row,[bool]$Code) {
    $address=[long]$capture.module_base+[long]$Row.rva
    $region=[ReadOnlyRangeProbe]::Query($proc.Id,$address)
    [ReadOnlyRangeProbe]::RequireImageRegion($region,$address,$Row.size,$capture.module_base,$Code)
    return [pscustomobject]@{region=$region;utc=[DateTime]::UtcNow.ToString('o');bytes=[ReadOnlyRangeProbe]::Read($proc.Id,$address,$Row.size)}
}
$tableRow=[pscustomobject]@{rva=$dir.RelativeVirtualAddress;size=$dir.Size}
$tableNow=Read-BoundRange $tableRow $false
if ([Convert]::ToHexString($tableNow.bytes) -cne [Convert]::ToHexString($table)) {throw 'Current live exception table differs'}
$findings=[Collections.Generic.List[object]]::new()
foreach ($snapshot in $snapshots) {
    $before=Read-BoundRange $snapshot.row $true
    if ([Convert]::ToHexString($before.bytes) -cne [Convert]::ToHexString($snapshot.bytes)) {throw 'Current captured code differs before engine'}
    $dir=Join-Path $out ("range-"+$snapshot.index);[void](New-Item -ItemType Directory $dir)
    $address=[long]$capture.module_base+[long]$snapshot.row.rva
    $result=Invoke-NativeRangeEngine $ToolDirectory $JavaHome $source $snapshot.file $address $dir
    if ($result.snapshot_sha256 -cne $snapshot.row.before_sha256 -or
        (Get-FileHash $snapshot.file).Hash.ToLowerInvariant() -cne $snapshot.row.before_sha256) {
        throw 'Engine input differs from source/process-bound captured range'
    }
    $after=Read-BoundRange $snapshot.row $true
    if ([Convert]::ToHexString($after.bytes) -cne [Convert]::ToHexString($snapshot.bytes)) {throw 'Current captured code differs after engine'}
    $findings.Add([pscustomobject]@{index=$snapshot.index;rva=$snapshot.row.rva;size=$snapshot.row.size;engine_receipt=$result;
        before_utc=$before.utc;after_utc=$after.utc;before_region=$before.region;after_region=$after.region;current_complete_bytes_equal_capture=$true})
}
$tableAfter=Read-BoundRange $tableRow $false
if ([Convert]::ToHexString($tableAfter.bytes) -cne [Convert]::ToHexString($table)) {throw 'Final live exception table differs'}
[void](Get-BoundCurrentParent)
$receipt=[ordered]@{verdict='PASS';qualification='source_bound_isolated_memory_research_only';source_sha256=$hash;
    source_content_identity=$fresh.content_identity;process_id=$proc.Id;process_start_utc=$capture.process_start_utc;module_base=$capture.module_base;
    module_memory_size=$capture.module_memory_size;findings=$findings;capture_receipt_sha256=(Get-FileHash ($captured+'/parent-live-ranges.json')).Hash;
    source_manifest_sha256=(Get-FileHash $SourceManifest).Hash;conformance_sha256=(Get-FileHash $ConformanceReceipt).Hash;
    admission_script_sha256=(Get-FileHash $PSCommandPath).Hash;scanner_sha256=(Get-FileHash $scanner).Hash;
    helper_sha256=(Get-FileHash (Join-Path $PSScriptRoot 'ReadOnlyRangeProbe.cs')).Hash;record_reader_sha256=(Get-FileHash (Join-Path $PSScriptRoot 'NativeReader.ps1')).Hash;
    complete_current_code_equal_capture=$true;complete_current_table_equal_source=$true;custom_code_memory_read=$true;
    process_memory_modified=$false;atomic_snapshot=$false;loaded_image_identity_verified=$false;active_calls_verified=$false;
    limitations='Fresh full selected-source scan and finite current repeated ranges only; decompiler pseudocode/unmapped references are leads; no source support or game-rule equivalence'}
Write-NewResearchFile ($out+'/admission-receipt.json') ([Text.Encoding]::UTF8.GetBytes(($receipt|ConvertTo-Json -Depth 9)))
Write-Output "PASS $($findings.Count) freshly source/process-bound isolated memory findings."
