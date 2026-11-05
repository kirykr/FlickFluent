<#
Run the benchmark as if the computer were the minimum spec (Windows).

  powershell -File bench\simulate-limits.ps1 [-Cores 4] [-RamGb 8] [-Bin <flickfluent.exe>] [-BenchArgs ...]

The program and everything it starts go into a Job Object that holds them to `-Cores` processors
(affinity) and `-RamGb` of memory for the whole job. A run that passes is proven to fit; a run the
system stops for memory is proven not to. The record says `simulated` either way: a real 8 GB
machine also runs Windows and other apps, and its CPU is slower than this one.

NOT RUN YET: written for the Windows benchmark pass (task T075) and never executed.
#>
param(
  [int]$Cores = 4,
  [double]$RamGb = 8,
  [string]$Bin = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\debug\flickfluent.exe'),
  [string[]]$BenchArgs = @()
)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path $Bin)) { throw "no program at $Bin (build it, or pass -Bin)" }

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
  public static IntPtr Create(int cores, ulong memoryBytes) {
    IntPtr job = CreateJobObject(IntPtr.Zero, null);
    var info = new EXTENDED();
    // 0x10 = affinity, 0x200 = memory limit for the whole job.
    info.Basic.LimitFlags = 0x10 | 0x200;
    info.Basic.Affinity = (UIntPtr)((1UL << cores) - 1);
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

$job = [JobLimit]::Create($Cores, [uint64]($RamGb * 1GB))
$benchArgsAll = @('--bench', '--simulate-cores', $Cores, '--simulate-ram-gb', $RamGb) + $BenchArgs
$p = Start-Process -FilePath $Bin -ArgumentList $benchArgsAll -NoNewWindow -PassThru -Wait:$false
[JobLimit]::Add($job, $p.Handle)
$p.WaitForExit()
Write-Host "exit code $($p.ExitCode); limits: $Cores cores, $RamGb GB (job memory)"
exit $p.ExitCode
