# Drag from (x1,y1) to (x2,y2), window-relative pixels (same coords as debug screenshots).
# Usage: powershell -File scripts/drag.ps1 <x1> <y1> <x2> <y2>
param([int]$X1, [int]$Y1, [int]$X2, [int]$Y2)
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class Dr {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte sc, uint fl, IntPtr e);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
'@
[void][Dr]::SetProcessDPIAware()
$p = Get-Process rimsort-rs -ErrorAction SilentlyContinue | Where-Object MainWindowHandle -ne 0 | Select-Object -First 1
if (-not $p) { Write-Error 'rimsort-rs window not found'; exit 1 }
$r = New-Object Dr+RECT
[void][Dr]::GetWindowRect($p.MainWindowHandle, [ref]$r)
[Dr]::keybd_event(0x12, 0, 0, [IntPtr]::Zero); [Dr]::keybd_event(0x12, 0, 2, [IntPtr]::Zero)
[void][Dr]::BringWindowToTop($p.MainWindowHandle); [void][Dr]::SetForegroundWindow($p.MainWindowHandle)
Start-Sleep -Milliseconds 150
[void][Dr]::SetCursorPos($r.L + $X1, $r.T + $Y1); Start-Sleep -Milliseconds 80
[Dr]::mouse_event(2, 0, 0, 0, [IntPtr]::Zero)          # left down
$steps = 20
for ($i = 1; $i -le $steps; $i++) {                    # HTML5 DnD needs real motion
  [void][Dr]::SetCursorPos($r.L + $X1 + [int](($X2 - $X1) * $i / $steps), $r.T + $Y1 + [int](($Y2 - $Y1) * $i / $steps))
  Start-Sleep -Milliseconds 25
}
Start-Sleep -Milliseconds 150
[Dr]::mouse_event(4, 0, 0, 0, [IntPtr]::Zero)          # left up
