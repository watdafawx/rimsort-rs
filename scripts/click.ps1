# Click at window-relative pixel (x,y) in the running RimSort-rs window (coords match debug screenshots).
# Usage: powershell -File scripts/click.ps1 <x> <y>
param([int]$X, [int]$Y, [int]$Count = 1)
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class Ui {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte sc, uint fl, IntPtr e);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
}
'@
[void][Ui]::SetProcessDPIAware()
$p = Get-Process rimsort-rs -ErrorAction SilentlyContinue | Where-Object MainWindowHandle -ne 0 | Select-Object -First 1
if (-not $p) { Write-Error 'rimsort-rs window not found'; exit 1 }
$r = New-Object Ui+RECT
[void][Ui]::GetWindowRect($p.MainWindowHandle, [ref]$r)
[Ui]::keybd_event(0x12, 0, 0, [IntPtr]::Zero); [Ui]::keybd_event(0x12, 0, 2, [IntPtr]::Zero) # ALT tap lifts the foreground lock
[void][Ui]::BringWindowToTop($p.MainWindowHandle)
[void][Ui]::SetForegroundWindow($p.MainWindowHandle)
Start-Sleep -Milliseconds 150
[void][Ui]::SetCursorPos($r.L + $X, $r.T + $Y)
Start-Sleep -Milliseconds 50
1..$Count | ForEach-Object { [Ui]::mouse_event(2, 0, 0, 0, [IntPtr]::Zero); [Ui]::mouse_event(4, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 60 }
