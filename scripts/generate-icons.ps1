Add-Type -AssemblyName System.Drawing

$ErrorActionPreference = "Stop"
$outDir = Resolve-Path (Join-Path $PSScriptRoot "..\src-tauri\icons")

function New-RoundedRect {
    param(
        [single]$X,
        [single]$Y,
        [single]$Width,
        [single]$Height,
        [single]$Radius
    )

    $diameter = $Radius * 2
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $path.AddArc($X, $Y, $diameter, $diameter, 180, 90)
    $path.AddArc($X + $Width - $diameter, $Y, $diameter, $diameter, 270, 90)
    $path.AddArc($X + $Width - $diameter, $Y + $Height - $diameter, $diameter, $diameter, 0, 90)
    $path.AddArc($X, $Y + $Height - $diameter, $diameter, $diameter, 90, 90)
    $path.CloseFigure()
    return $path
}

function Fill-RoundedRect {
    param(
        [System.Drawing.Graphics]$Graphics,
        [System.Drawing.Brush]$Brush,
        [single]$X,
        [single]$Y,
        [single]$Width,
        [single]$Height,
        [single]$Radius
    )

    $path = New-RoundedRect $X $Y $Width $Height $Radius
    $Graphics.FillPath($Brush, $path)
    $path.Dispose()
}

function New-PureWallBitmap {
    param([int]$Size)

    $scale = $Size / 256.0
    $bmp = New-Object System.Drawing.Bitmap $Size, $Size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $graphics = [System.Drawing.Graphics]::FromImage($bmp)
    $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $graphics.Clear([System.Drawing.Color]::Transparent)

    function S([double]$value) {
        return [single]($value * $scale)
    }

    $bgTop = [System.Drawing.Color]::FromArgb(255, 251, 253, 255)
    $bgBottom = [System.Drawing.Color]::FromArgb(255, 237, 246, 255)
    $border = [System.Drawing.Color]::FromArgb(255, 211, 229, 247)
    $backCard = [System.Drawing.Color]::FromArgb(255, 227, 241, 255)
    $midCard = [System.Drawing.Color]::FromArgb(255, 251, 253, 255)
    $midStroke = [System.Drawing.Color]::FromArgb(255, 214, 230, 247)
    $photoTop = [System.Drawing.Color]::FromArgb(255, 107, 184, 241)
    $photoBottom = [System.Drawing.Color]::FromArgb(255, 244, 250, 255)
    $snow = [System.Drawing.Color]::FromArgb(246, 255, 255, 255)
    $ridge = [System.Drawing.Color]::FromArgb(158, 74, 111, 145)
    $ink = [System.Drawing.Color]::FromArgb(255, 74, 111, 145)
    $accentTop = [System.Drawing.Color]::FromArgb(255, 35, 139, 230)
    $accentBottom = [System.Drawing.Color]::FromArgb(255, 9, 111, 200)

    $outer = New-RoundedRect (S 8) (S 8) (S 240) (S 240) (S 54)
    $bgRect = [System.Drawing.RectangleF]::new((S 8), (S 8), (S 240), (S 240))
    $bgBrush = [System.Drawing.Drawing2D.LinearGradientBrush]::new($bgRect, $bgTop, $bgBottom, [single]45)
    $graphics.FillPath($bgBrush, $outer)
    $bgBrush.Dispose()

    $borderPen = New-Object System.Drawing.Pen $border, ([Math]::Max(1.0, (S 3)))
    $borderPen.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round
    $graphics.DrawPath($borderPen, $outer)

    $backBrush = New-Object System.Drawing.SolidBrush $backCard
    Fill-RoundedRect $graphics $backBrush (S 43) (S 48) (S 128) (S 124) (S 28)
    $backBrush.Dispose()

    $midPath = New-RoundedRect (S 56) (S 57) (S 128) (S 124) (S 28)
    $midBrush = New-Object System.Drawing.SolidBrush $midCard
    $graphics.FillPath($midBrush, $midPath)
    $midBrush.Dispose()
    $midPen = New-Object System.Drawing.Pen $midStroke, ([Math]::Max(1.0, (S 3)))
    $midPen.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round
    $graphics.DrawPath($midPen, $midPath)
    $midPen.Dispose()
    $midPath.Dispose()

    $front = New-RoundedRect (S 70) (S 66) (S 124) (S 124) (S 28)
    $photoRect = [System.Drawing.RectangleF]::new((S 70), (S 66), (S 124), (S 124))
    $photoBrush = [System.Drawing.Drawing2D.LinearGradientBrush]::new($photoRect, $photoTop, $photoBottom, [single]95)
    $graphics.FillPath($photoBrush, $front)
    $photoBrush.Dispose()

    $oldClip = $graphics.Clip
    $graphics.SetClip($front)

    if ($Size -ge 24) {
        $sunBrush = New-Object System.Drawing.SolidBrush $accentTop
        $graphics.FillEllipse($sunBrush, (S 90), (S 85), (S 22), (S 22))
        $sunBrush.Dispose()
    }

    $snowPath = New-Object System.Drawing.Drawing2D.GraphicsPath
    $snowPath.AddPolygon([System.Drawing.PointF[]]@(
        [System.Drawing.PointF]::new((S 70), (S 144)),
        [System.Drawing.PointF]::new((S 103), (S 116)),
        [System.Drawing.PointF]::new((S 132), (S 143)),
        [System.Drawing.PointF]::new((S 154), (S 124)),
        [System.Drawing.PointF]::new((S 194), (S 158)),
        [System.Drawing.PointF]::new((S 194), (S 190)),
        [System.Drawing.PointF]::new((S 70), (S 190))
    ))
    $snowBrush = New-Object System.Drawing.SolidBrush $snow
    $graphics.FillPath($snowBrush, $snowPath)
    $snowBrush.Dispose()
    $snowPath.Dispose()

    $ridgePath = New-Object System.Drawing.Drawing2D.GraphicsPath
    $ridgePath.AddPolygon([System.Drawing.PointF[]]@(
        [System.Drawing.PointF]::new((S 70), (S 160)),
        [System.Drawing.PointF]::new((S 110), (S 134)),
        [System.Drawing.PointF]::new((S 140), (S 158)),
        [System.Drawing.PointF]::new((S 160), (S 144)),
        [System.Drawing.PointF]::new((S 194), (S 166)),
        [System.Drawing.PointF]::new((S 194), (S 190)),
        [System.Drawing.PointF]::new((S 70), (S 190))
    ))
    $ridgeBrush = New-Object System.Drawing.SolidBrush $ridge
    $graphics.FillPath($ridgeBrush, $ridgePath)
    $ridgeBrush.Dispose()
    $ridgePath.Dispose()

    $graphics.Clip = $oldClip
    $oldClip.Dispose()

    $frontPen = New-Object System.Drawing.Pen $ink, ([Math]::Max(2.0, (S 6)))
    $frontPen.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round
    $graphics.DrawPath($frontPen, $front)
    $frontPen.Dispose()

    $chevronRect = [System.Drawing.RectangleF]::new((S 181), (S 104), (S 32), (S 48))
    $chevronBrush = [System.Drawing.Drawing2D.LinearGradientBrush]::new($chevronRect, $accentTop, $accentBottom, [single]90)
    $chevronPen = New-Object System.Drawing.Pen $chevronBrush, ([Math]::Max(2.8, (S 13)))
    $chevronPen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $chevronPen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $chevronPen.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round
    $graphics.DrawLines($chevronPen, [System.Drawing.PointF[]]@(
        [System.Drawing.PointF]::new((S 188), (S 111)),
        [System.Drawing.PointF]::new((S 207), (S 128)),
        [System.Drawing.PointF]::new((S 188), (S 145))
    ))
    $chevronPen.Dispose()
    $chevronBrush.Dispose()

    $borderPen.Dispose()
    $front.Dispose()
    $outer.Dispose()
    $graphics.Dispose()
    return $bmp
}

function Save-Png {
    param([int]$Size, [string]$Path)
    $bmp = New-PureWallBitmap $Size
    $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}

function Save-Ico {
    param([int]$Size, [string]$Path)

    $bmp = New-PureWallBitmap $Size
    $icon = [System.Drawing.Icon]::FromHandle($bmp.GetHicon())
    $stream = [System.IO.File]::Create($Path)
    $icon.Save($stream)
    $stream.Dispose()
    $icon.Dispose()
    $bmp.Dispose()
}

Save-Png 16 (Join-Path $outDir "16x16.png")
Save-Png 24 (Join-Path $outDir "24x24.png")
Save-Png 32 (Join-Path $outDir "32x32.png")
Save-Png 128 (Join-Path $outDir "128x128.png")
Save-Png 256 (Join-Path $outDir "icon.png")
Save-Png 256 (Join-Path $outDir "128x128@2x.png")
Save-Ico 64 (Join-Path $outDir "icon.ico")

Write-Host "Generated PureWall icons in $outDir"
