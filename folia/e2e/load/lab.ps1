# The lab of the load tests (docs/folia/frontend.md, "Load"): Radix serves a snapshot, Folia a release
# build of this checkout with the production settings, pinned to as many processors as its
# container gets on the server, the access gate off, the load tool on other processors.
# Windows PowerShell, from the repository root:
#
#   . .\e2e\load\lab.ps1 -Snapshot C:\lab\snapshot -Folia C:\lab\folia.exe -Radix C:\lab\radix.exe
#   Start-Lab -Cpus 3 -FoliaMask 0x54            # three logical processors on three cores
#   Invoke-Load -Name mix -LoadArgs @("run", "-base", "http://127.0.0.1:18080", "-scenario", "crawl-cold", "-pages", "pages.tsv", "-rates", "50,100")
#   Stop-Lab
#
# Windows does not tell Rust about the affinity (the CPU limit of a container does on Linux), so
# the processors are also given to Folia explicitly (FOLIA_WORKERS, FOLIA_RENDER_PLACES,
# FOLIA_FEED_PLACES). The workstation's processors are faster than the server's: calibrate with
# the same calendar feeds on both (docs/folia/frontend.md).
param(
    [Parameter(Mandatory = $true)][string]$Snapshot,
    [Parameter(Mandatory = $true)][string]$Folia,
    [Parameter(Mandatory = $true)][string]$Radix,
    [string]$Lab = (Join-Path $env:TEMP "betula-lab"),
    [string]$Load = (Join-Path $PSScriptRoot "betula-load.exe")
)

$Site = Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) "site"
New-Item -ItemType Directory -Force $Lab | Out-Null

function Start-Radix {
    if (Get-NetTCPConnection -State Listen -LocalPort 18090 -ErrorAction SilentlyContinue) { return }
    $p = Start-Process -FilePath $Radix -ArgumentList @("serve-snapshot", "--addr", "127.0.0.1:18090", "--dir", $Snapshot) `
        -RedirectStandardOutput "$Lab\radix.out.log" -RedirectStandardError "$Lab\radix.err.log" -WindowStyle Hidden -PassThru
    $p.Id | Out-File -Encoding ascii "$Lab\radix.pid"
    $p.ProcessorAffinity = [IntPtr]0x3
}

function Start-Lab {
    param([int]$Cpus = 1, [long]$FoliaMask = 0x4, [int]$CacheMB = 256, [string]$Tag = "run", [switch]$KeepData, [string]$Warm = "off", [int]$RenderWaitMs = 3000)
    Start-Radix
    Stop-Folia
    if (-not $KeepData) { Remove-Item -Recurse -Force "$Lab\web-data" -ErrorAction SilentlyContinue }
    $env:FOLIA_ADDR = "127.0.0.1:18080"
    $env:FOLIA_SNAPSHOT_URL = "http://127.0.0.1:18090/snapshot/catalog.db"
    $env:FOLIA_DATA_DIR = "$Lab\web-data"
    $env:FOLIA_SITE_ROOT = $Site
    $env:FOLIA_PUBLIC_URL = "http://127.0.0.1:18080"
    $env:FOLIA_LOG_FORMAT = "json"
    $env:FOLIA_LOG_LEVEL = "info"
    $env:FOLIA_HTML_CACHE_MB = "$CacheMB"
    $env:FOLIA_ACCESS_GATE = "off"
    $env:FOLIA_WARM_CACHE = $Warm
    $env:FOLIA_RENDER_WAIT_MS = "$RenderWaitMs"
    $env:FOLIA_WORKERS = "$($Cpus + 1)"
    $env:FOLIA_RENDER_PLACES = "$Cpus"
    $env:FOLIA_FEED_PLACES = "$Cpus"
    $p = Start-Process -FilePath $Folia -RedirectStandardOutput "$Lab\folia-$Tag.log" -RedirectStandardError "$Lab\folia-$Tag.err.log" -WindowStyle Hidden -PassThru
    $p.ProcessorAffinity = [IntPtr]$FoliaMask
    $p.Id | Out-File -Encoding ascii "$Lab\folia.pid"
    # Ready once a snapshot is active (it downloads and compresses 44 MB first).
    $deadline = (Get-Date).AddSeconds(120)
    while ((Get-Date) -lt $deadline) {
        try {
            $r = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:18080/healthz" -TimeoutSec 2
            if ($r.StatusCode -eq 200) { Write-Output "folia ready pid=$($p.Id) cpus=$Cpus mask=0x$('{0:X}' -f $FoliaMask)"; return }
        } catch {}
        Start-Sleep -Milliseconds 500
    }
    throw "folia did not become healthy (see $Lab\folia-$Tag.log)"
}

# The load tool on the other cores (0xFC0 = logical processors 6-11), with the server's process
# for its CPU and memory figures.
function Invoke-Load {
    param([string[]]$LoadArgs, [string]$Name, [long]$Mask = 0xFC0)
    $out = "$Lab\out-$Name.txt"
    $pidArg = @()
    if (Test-Path "$Lab\folia.pid") { $pidArg = @("-pid", (Get-Content "$Lab\folia.pid").Trim()) }
    $all = @($LoadArgs[0]) + $pidArg + @("-name", $Name, "-out", "$Lab\results.jsonl") + $LoadArgs[1..($LoadArgs.Length - 1)]
    $p = Start-Process -FilePath $Load -ArgumentList $all -WorkingDirectory (Get-Location) `
        -RedirectStandardOutput $out -RedirectStandardError "$out.err" -WindowStyle Hidden -PassThru
    try { $p.ProcessorAffinity = [IntPtr]$Mask } catch {}
    $p.WaitForExit()
    Get-Content $out
    Get-Content "$out.err" | Select-Object -First 5
}

function Stop-Folia {
    if (Test-Path "$Lab\folia.pid") {
        Stop-Process -Id ([int](Get-Content "$Lab\folia.pid")) -Force -ErrorAction SilentlyContinue
        Remove-Item "$Lab\folia.pid"
        Start-Sleep -Milliseconds 500
    }
}

function Stop-Lab {
    Stop-Folia
    if (Test-Path "$Lab\radix.pid") {
        Stop-Process -Id ([int](Get-Content "$Lab\radix.pid")) -Force -ErrorAction SilentlyContinue
        Remove-Item "$Lab\radix.pid"
    }
}
