$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

# Local, redistributable Liberation Sans; no system/proprietary font substitution.
$fontDirectory = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../app/security-runtime/node_modules/pdfjs-dist/standard_fonts'))
$fontPath = Join-Path $fontDirectory 'LiberationSans-Bold.ttf'
if (-not (Test-Path -LiteralPath $fontPath)) { throw "Required source font missing: $fontPath" }
$collection = [Drawing.Text.PrivateFontCollection]::new()
$collection.AddFontFile($fontPath)
$family = $collection.Families[0]
if ($family.Name -ne 'Liberation Sans') { throw "Unexpected font family: $($family.Name)" }
$outline = [Drawing.Drawing2D.GraphicsPath]::new()
$outline.AddString('OpenGlitch', $family, [int][Drawing.FontStyle]::Bold, 100, [Drawing.PointF]::new(0, 0), [Drawing.StringFormat]::GenericTypographic)
$bounds = $outline.GetBounds()
$matrix = [Drawing.Drawing2D.Matrix]::new()
$scale = 31.0 / $bounds.Height
$matrix.Translate(-$bounds.X, -$bounds.Y)
$matrix.Scale($scale, $scale, [Drawing.Drawing2D.MatrixOrder]::Append)
$outline.Transform($matrix)
$points = $outline.PathPoints
$types = $outline.PathTypes
$culture = [Globalization.CultureInfo]::InvariantCulture
function Number([double] $n) { $n.ToString('0.###', $culture) }
function Point([Drawing.PointF] $p) { "$(Number $p.X) $(Number $p.Y)" }
$commands = [Collections.Generic.List[string]]::new()
for ($i = 0; $i -lt $points.Length; $i++) {
  $type = $types[$i] -band 7
  if ($type -eq 0) { $commands.Add("M$(Point $points[$i])") }
  elseif ($type -eq 1) { $commands.Add("L$(Point $points[$i])") }
  elseif ($type -eq 3) {
    $commands.Add("C$(Point $points[$i]) $(Point $points[$i+1]) $(Point $points[$i+2])")
    $i += 2
  }
  else { throw "Unsupported outline segment: $type" }
  if ($types[$i] -band 128) { $commands.Add('Z') }
}
$wordPath = $commands -join ''
$wordWidth = [Math]::Ceiling($outline.GetBounds().Width)
$ink = '#202124'
$paper = '#f5f4ef'
$violet = '#6942e7'
$violetLight = '#b8a4ff'
$symbolBody = '<path d="M3 6h16v8h-8v8h16v8H3Z"/>'
$symbolShift = '<path d="M21 4h8v8h-8Z"/>'
$symbol = $symbolBody + $symbolShift
$utf8 = [Text.UTF8Encoding]::new($false)
function Write-Svg([string] $name, [string] $viewBox, [string] $contents, [string] $label) {
  $svg = "<svg xmlns=`"http://www.w3.org/2000/svg`" viewBox=`"$viewBox`" role=`"img`" aria-label=`"$label`">$contents</svg>`n"
  [IO.File]::WriteAllText((Join-Path $PSScriptRoot $name), $svg, $utf8)
}

Write-Svg 'openglitch-symbol.svg' '0 0 32 32' "<g fill=`"currentColor`">$symbol</g>" 'OpenGlitch'
Write-Svg 'openglitch-symbol-ink.svg' '0 0 32 32' "<g fill=`"$ink`">$symbol</g>" 'OpenGlitch'
Write-Svg 'openglitch-symbol-paper.svg' '0 0 32 32' "<g fill=`"$paper`">$symbol</g>" 'OpenGlitch'
Write-Svg 'openglitch-symbol-accent.svg' '0 0 32 32' "<g fill=`"$ink`">$symbolBody</g><g fill=`"$violet`">$symbolShift</g>" 'OpenGlitch'
Write-Svg 'openglitch-symbol-accent-paper.svg' '0 0 32 32' "<g fill=`"$paper`">$symbolBody</g><g fill=`"$violetLight`">$symbolShift</g>" 'OpenGlitch'

$word = "<path d=`"$wordPath`" fill-rule=`"evenodd`"/>"
Write-Svg 'openglitch-wordmark.svg' "0 0 $wordWidth 31" "<g fill=`"currentColor`">$word</g>" 'OpenGlitch'
$lockWidth = 46 + $wordWidth
$lockBody = "<g transform=`"translate(0 -3)`">$symbol</g><g transform=`"translate(46 2)`">$word</g>"
Write-Svg 'openglitch-lockup.svg' "0 0 $lockWidth 36" "<g fill=`"currentColor`">$lockBody</g>" 'OpenGlitch'
Write-Svg 'openglitch-lockup-ink.svg' "0 0 $lockWidth 36" "<g fill=`"$ink`">$lockBody</g>" 'OpenGlitch'
Write-Svg 'openglitch-lockup-paper.svg' "0 0 $lockWidth 36" "<g fill=`"$paper`">$lockBody</g>" 'OpenGlitch'

# Opaque paper surface keeps contrast independent of browser chrome/theme.
Write-Svg 'favicon.svg' '0 0 32 32' "<path fill=`"$paper`" d=`"M0 0h32v32H0Z`"/><g fill=`"$ink`">$symbol</g>" 'OpenGlitch'
Write-Svg 'favicon-24.svg' '0 0 24 24' "<path fill=`"$paper`" d=`"M0 0h24v24H0Z`"/><path fill=`"$ink`" d=`"M2 4h12v6H8v6h12v6H2ZM16 2h6v6h-6Z`"/>" 'OpenGlitch'
Write-Svg 'favicon-16.svg' '0 0 16 16' "<path fill=`"$paper`" d=`"M0 0h16v16H0Z`"/><path fill=`"$ink`" d=`"M1 3h8v4H5v4h8v4H1ZM11 1h4v4h-4Z`"/>" 'OpenGlitch'

[IO.File]::Copy((Join-Path $fontDirectory 'LICENSE_LIBERATION'), (Join-Path $PSScriptRoot 'LICENSE-Liberation.txt'), $true)
$sha = (Get-FileHash -LiteralPath $fontPath -Algorithm SHA256).Hash.ToLowerInvariant()
$manifest = [ordered]@{
  brand = 'OpenGlitch'; product = 'OpenKoda'; generated = '2026-11-14';
  symbol = @{ geometry = 'Original seven-module open portal'; viewBox = '0 0 32 32'; module = 8; displacement = @(2,-2) };
  wordmark = @{ source = 'LiberationSans-Bold.ttf'; sourcePackage = 'pdfjs-dist/standard_fonts'; sha256 = $sha; outlineMethod = 'System.Drawing GraphicsPath, PrivateFontCollection'; height = 31; width = $wordWidth; fontFamily = $family.Name; license = 'GPL-2.0 with Liberation document embedding exception; see LICENSE-Liberation.txt' };
  colors = @{ ink = $ink; paper = $paper; accent = $violet; accentOnDark = $violetLight };
  lockup = @{ width = $lockWidth; height = 36; wordmarkX = 46; wordmarkY = 2; symbolY = -3 };
}
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'provenance.json'), ($manifest | ConvertTo-Json -Depth 5), $utf8)
$outline.Dispose(); $matrix.Dispose(); $collection.Dispose()
Write-Output "Generated OpenGlitch assets: wordmark $($wordWidth)x31, lockup $($lockWidth)x36."
