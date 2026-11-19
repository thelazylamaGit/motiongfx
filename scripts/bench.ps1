<#
.SYNOPSIS
Runs `cargo bench` pinned to the CPU's performance cores, so timings vary
less between runs.

.DESCRIPTION
On a hybrid CPU, Windows can run a benchmark on a fast or an efficient core,
or move it between them mid-run. This script finds the most performant
cores, pins itself to them and runs `cargo bench` with your arguments.
Child processes, including the benchmark binary, inherit the pinning.

It doesn't remove other sources of noise: close other heavy programs
(including editor background builds), and compare against a baseline
recorded in the same session.

.EXAMPLE
./scripts/bench.ps1 -p vello_motiongfx --bench authoring -- --save-baseline before playback

.EXAMPLE
./scripts/bench.ps1 -p vello_motiongfx --bench authoring -- --baseline before playback
#>

$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;

public static class CpuSets {
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool GetSystemCpuSetInformation(
        IntPtr information, uint length, out uint returned,
        IntPtr process, uint flags);

    // An affinity mask of the logical processors in group 0 with the
    // highest efficiency class: the performance cores. On a CPU with one
    // kind of core, that is every core.
    public static long PerformanceMask() {
        uint length;
        GetSystemCpuSetInformation(IntPtr.Zero, 0, out length, IntPtr.Zero, 0);
        IntPtr buffer = Marshal.AllocHGlobal((int)length);
        try {
            if (!GetSystemCpuSetInformation(buffer, length, out length, IntPtr.Zero, 0)) {
                throw new Win32Exception();
            }
            long mask = 0;
            int best = -1;
            int offset = 0;
            while (offset < length) {
                // SYSTEM_CPU_SET_INFORMATION: Size at 0, Group at 12,
                // LogicalProcessorIndex at 14, EfficiencyClass at 18.
                int size = Marshal.ReadInt32(buffer, offset);
                short group = Marshal.ReadInt16(buffer, offset + 12);
                int logical = Marshal.ReadByte(buffer, offset + 14);
                int efficiency = Marshal.ReadByte(buffer, offset + 18);
                if (group == 0 && logical < 64) {
                    if (efficiency > best) {
                        best = efficiency;
                        mask = 0;
                    }
                    if (efficiency == best) {
                        mask |= 1L << logical;
                    }
                }
                offset += size;
            }
            return mask;
        } finally {
            Marshal.FreeHGlobal(buffer);
        }
    }
}
'@

$mask = [CpuSets]::PerformanceMask()
$cores = (0..63 | Where-Object { $mask -band (1L -shl $_) }) -join ', '
Write-Host "Pinning cargo bench to logical processors $cores"

$process = [System.Diagnostics.Process]::GetCurrentProcess()
$previous = $process.ProcessorAffinity
try {
    $process.ProcessorAffinity = [IntPtr]$mask
    & cargo bench @args
    $code = $LASTEXITCODE
} finally {
    $process.ProcessorAffinity = $previous
}
exit $code
