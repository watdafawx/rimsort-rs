# Screenshot the running RimSort-rs window -> debug/screenshots/<name>.png
# Usage: powershell -File scripts/shot.ps1 [name]   (prints the path)
param([string]$Name = (Get-Date -Format 'yyyyMMdd-HHmmss'))
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class Win {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
'@
[void][Win]::SetProcessDPIAware()
$p = Get-Process rimsort-rs -ErrorAction SilentlyContinue | Where-Object MainWindowHandle -ne 0 | Select-Object -First 1
if (-not $p) { Write-Error 'rimsort-rs window not found'; exit 1 }
$r = New-Object Win+RECT
[void][Win]::GetWindowRect($p.MainWindowHandle, [ref]$r)
$bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$dc = $g.GetHdc()
[void][Win]::PrintWindow($p.MainWindowHandle, $dc, 2) # PW_RENDERFULLCONTENT: captures WebView2 content
$g.ReleaseHdc($dc)
$dir = Join-Path $PSScriptRoot '..\debug\screenshots'
New-Item -ItemType Directory -Force $dir | Out-Null
$out = Join-Path (Resolve-Path $dir) "$Name.png"
$bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
$out
