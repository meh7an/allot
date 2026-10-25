# Builds github/social-preview.png (1280 x 640), the repository's link preview: the mark and the
# name on ink. Needs Windows for System.Drawing and Segoe UI; run from anywhere:
#
#     powershell -NoProfile -File assets/brand/social-preview.ps1
#
# GitHub takes it only through the web UI: Settings, General, Social preview, Edit, Upload.

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$brand = $PSScriptRoot
$out = Join-Path $brand 'github\social-preview.png'
New-Item -ItemType Directory -Force -Path (Split-Path $out) | Out-Null

$bitmap = New-Object System.Drawing.Bitmap 1280, 640
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.SmoothingMode = 'AntiAlias'
$graphics.InterpolationMode = 'HighQualityBicubic'
$graphics.PixelOffsetMode = 'HighQuality'
$graphics.TextRenderingHint = 'AntiAlias'
$graphics.Clear([System.Drawing.ColorTranslator]::FromHtml('#16171B'))

$mark = [System.Drawing.Image]::FromFile((Join-Path $brand 'png\allot-mark-color-dark-4x.png'))
$graphics.DrawImage($mark, 150, 160, 320, 320)

$title = New-Object System.Drawing.Font 'Segoe UI Semibold', 132, ([System.Drawing.GraphicsUnit]::Pixel)
$tagline = New-Object System.Drawing.Font 'Segoe UI', 38, ([System.Drawing.GraphicsUnit]::Pixel)
$white = New-Object System.Drawing.SolidBrush ([System.Drawing.ColorTranslator]::FromHtml('#F4F4F6'))
$grey = New-Object System.Drawing.SolidBrush ([System.Drawing.ColorTranslator]::FromHtml('#A3A6B0'))
$graphics.DrawString('Allot', $title, $white, 520, 172)
$graphics.DrawString("Coordinates CPU and GPU heavy work`nbetween Claude Code sessions", $tagline, $grey, 528, 360)

$bitmap.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
foreach ($disposable in $graphics, $bitmap, $mark, $title, $tagline, $white, $grey) {
    $disposable.Dispose()
}
Write-Output $out
