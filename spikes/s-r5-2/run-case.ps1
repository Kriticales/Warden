# Roda um caso do spike S-R5-2: abre o Alloc.java numa JVM, passa por etapas de
# memória e, em cada uma, compara o leitor (hsperf-probe) com o jstat do JDK.
#
#   ./run-case.ps1 -Java C:\wt\s-r5-2\j\21\bin\java.exe -Label j21-g1 -JvmArgs '-Xmx1024m','-XX:+UseG1GC'
#
# Opcional: -Env @{ USERNAME = 'João' } muda variáveis só do processo da JVM;
# -Fixture <arquivo> grava o hsperfdata na etapa "hold200".
param(
    [Parameter(Mandatory)] [string] $Java,
    [Parameter(Mandatory)] [string] $Label,
    [string[]] $JvmArgs = @('-Xmx1024m'),
    [hashtable] $Env = @{},
    [string] $Fixture = '',
    [string] $Jstat = 'C:\wt\s-r5-2\j\jdk\bin\jstat.exe',
    [string] $Classes = 'C:\wt\s-r5-2\cls',
    [string] $Probe = "$PSScriptRoot\target\release\hsperf-probe.exe",
    [string] $TempDir = '',
    [switch] $NoCompare
)
$ErrorActionPreference = 'Stop'

$psi = [System.Diagnostics.ProcessStartInfo]::new($Java)
foreach ($a in $JvmArgs) { $psi.ArgumentList.Add($a) }
$psi.ArgumentList.Add('-cp'); $psi.ArgumentList.Add($Classes); $psi.ArgumentList.Add('Alloc')
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.UseShellExecute = $false
foreach ($k in $Env.Keys) { $psi.Environment[$k] = $Env[$k] }
$p = [System.Diagnostics.Process]::Start($psi)
$line = $p.StandardOutput.ReadLine()
if ($line -notmatch '^pid (\d+)$') { throw "saída inesperada: $line / $($p.StandardError.ReadToEnd())" }
$jpid = [int]$Matches[1]
Write-Host "# $Label pid=$jpid args=$($JvmArgs -join ' ')"

$tempArgs = @(); if ($TempDir) { $tempArgs = @('--temp', $TempDir) }

function Send([string] $cmd) {
    $p.StandardInput.WriteLine($cmd); $p.StandardInput.Flush()
    $r = $p.StandardOutput.ReadLine()
    if ($r -notlike 'ok*') { throw "resposta inesperada a '$cmd': $r" }
}
function Measure-Step([string] $step, [int] $count = 3, [int] $interval = 300) {
    if ($NoCompare) { return }
    & $Probe compare $jpid --jstat $Jstat --count $count --interval-ms $interval --label "$Label/$step" @tempArgs
}

try {
    Start-Sleep -Milliseconds 1500
    & $Probe read $jpid @tempArgs
    Measure-Step 'inicio'
    Send 'hold 200'; Start-Sleep -Milliseconds 300
    Measure-Step 'hold200'
    if ($Fixture) { & $Probe dump $jpid $Fixture @tempArgs; & $Jstat -gc $jpid | Set-Content "$Fixture.jstat.txt" }
    Send 'garbage 300'; Start-Sleep -Milliseconds 300
    Measure-Step 'lixo300'
    Send 'gc'; Start-Sleep -Milliseconds 300
    Measure-Step 'gc'
    Send 'churn 8'
    Measure-Step 'churn' 8 500
}
finally {
    if (-not $p.HasExited) { $p.StandardInput.WriteLine('quit'); $p.StandardInput.Flush(); [void]$p.WaitForExit(5000) }
    if (-not $p.HasExited) { $p.Kill() }
    $err = $p.StandardError.ReadToEnd(); if ($err) { Write-Host "# stderr da JVM: $err" }
}
