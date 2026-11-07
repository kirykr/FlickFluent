<#
Run the benchmark as if the computer were the minimum spec (Windows).

  powershell -File bench\simulate-limits.ps1 [-Cores 4] [-RamGb 8] [-Bin <flickfluent.exe>] [-BenchArgs ...]

The program and everything it starts go into a Job Object that holds them to `-Cores` processors
(affinity) and `-RamGb` of memory for the whole job. A run that passes is proven to fit; a run the
system stops for memory is proven not to. The record says `simulated` either way: a real 8 GB
machine also runs Windows and other apps, and its CPU is slower than this one.

`-Cores` counts physical cores, as the hardware probe does. On a CPU with two threads per core,
Windows numbers both threads of a core next to each other, so the mask takes one logical processor
from each of the first `-Cores` cores (0, 2, 4, ...), not logical processors 0 to Cores-1 (which
would be only Cores/2 real cores).

The release build has no console, so its output goes to `-Out` (JSON when `--json` is passed).
#>
param(
  [int]$Cores = 4,
  [double]$RamGb = 8,
  [string]$Bin = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\release\flickfluent.exe'),
  [string]$Out = (Join-Path $env:TEMP 'flickfluent-bench-simulated.out'),
  [string[]]$BenchArgs = @()
)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path $Bin)) { throw "no program at $Bin (build it, or pass -Bin)" }

$cpu = @(Get-CimInstance Win32_Processor)
$physical = ($cpu | Measure-Object -Property NumberOfCores -Sum).Sum
$logical = ($cpu | Measure-Object -Property NumberOfLogicalProcessors -Sum).Sum
if ($Cores -gt $physical) { throw "this computer has $physical cores; cannot simulate $Cores" }
$step = [math]::Max(1, [int]($logical / $physical))
[uint64]$mask = 0
for ($i = 0; $i -lt $Cores; $i++) { $mask = $mask -bor ([uint64]1 -shl ($i * $step)) }

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class JobLimit {
  [StructLayout(LayoutKind.Sequential)] struct BASIC {
    public long PerProcessUserTimeLimit, PerJobUserTimeLimit; public uint LimitFlags;
    public UIntPtr MinimumWorkingSetSize, MaximumWorkingSetSize; public uint ActiveProcessLimit;
    public UIntPtr Affinity; public uint PriorityClass, SchedulingClass; }
  [StructLayout(LayoutKind.Sequential)] struct IO { public ulong a, b, c, d, e, f; }
  [StructLayout(LayoutKind.Sequential)] struct EXTENDED {
    public BASIC Basic; public IO Io; public UIntPtr ProcessMemoryLimit, JobMemoryLimit,
    PeakProcessMemoryUsed, PeakJobMemoryUsed; }
  [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr CreateJobObject(IntPtr a, string n);
  [DllImport("kernel32.dll", SetLastError = true)] static extern bool SetInformationJobObject(IntPtr j, int c, ref EXTENDED i, int l);
  [DllImport("kernel32.dll", SetLastError = true)] static extern bool AssignProcessToJobObject(IntPtr j, IntPtr p);
  public static IntPtr Create(ulong affinity, ulong memoryBytes) {
    IntPtr job = CreateJobObject(IntPtr.Zero, null);
    var info = new EXTENDED();
    // 0x10 = affinity, 0x200 = memory limit for the whole job.
    info.Basic.LimitFlags = 0x10 | 0x200;
    info.Basic.Affinity = (UIntPtr)affinity;
    info.JobMemoryLimit = (UIntPtr)memoryBytes;
    if (!SetInformationJobObject(job, 9, ref info, Marshal.SizeOf(typeof(EXTENDED))))
      throw new System.ComponentModel.Win32Exception();
    return job;
  }
  public static void Add(IntPtr job, IntPtr process) {
    if (!AssignProcessToJobObject(job, process)) throw new System.ComponentModel.Win32Exception();
  }
}
'@

$job = [JobLimit]::Create($mask, [uint64]($RamGb * 1GB))
$benchArgsAll = @('--bench', '--simulate-cores', $Cores, '--simulate-ram-gb', $RamGb) + $BenchArgs
# The worker processes start after setup, well after the program joins the job, so they inherit it.
$p = Start-Process -FilePath $Bin -ArgumentList $benchArgsAll -NoNewWindow -PassThru `
  -RedirectStandardOutput $Out -RedirectStandardError "$Out.err"
[JobLimit]::Add($job, $p.Handle)
$p.WaitForExit()
Write-Host ("exit code {0}; limits: {1} cores (affinity mask 0x{2:X}), {3} GB (job memory); output in {4}" -f `
  $p.ExitCode, $Cores, $mask, $RamGb, $Out)
exit $p.ExitCode
