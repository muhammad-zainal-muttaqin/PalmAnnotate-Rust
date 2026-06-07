[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
$ReadUtf8 = New-Object System.Text.UTF8Encoding($false, $true)
$WriteUtf8Bom = New-Object System.Text.UTF8Encoding($true)
$Paths = @(
    'README.md'
    'REQUIREMENTS.md'
    'docs\architecture.md'
    'docs\android-build.md'
)

foreach ($RelativePath in $Paths) {
    $Path = Join-Path $Root $RelativePath
    $Text = [IO.File]::ReadAllText($Path, $ReadUtf8)
    while ($Text.Length -gt 0 -and [int]$Text[0] -eq 0xFEFF) {
        $Text = $Text.Substring(1)
    }
    while (
        $Text.Length -ge 3 -and
        [int]$Text[0] -eq 0x00EF -and
        [int]$Text[1] -eq 0x00BB -and
        [int]$Text[2] -eq 0x00BF
    ) {
        $Text = $Text.Substring(3)
    }
    [IO.File]::WriteAllText($Path, $Text, $WriteUtf8Bom)
}
