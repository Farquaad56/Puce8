# Convertit en ASCII tous les fichiers .rs et .toml sous crates/ (UTF-8 sans BOM, fins de ligne conservees).
# Usage (depuis puce8/) : powershell -ExecutionPolicy Bypass -File scripts\ascii.ps1
# Fichier volontairement 100 % ASCII : les caracteres speciaux sont ecrits avec [char]0x....
$map = @(
    @(0x00AB, '"'),
    @(0x00BB, '"'),
    @(0x2014, '-'),
    @(0x2013, '-'),
    @(0x2026, '...'),
    @(0x0153, 'oe'),
    @(0x0152, 'OE'),
    @(0x00E6, 'ae'),
    @(0x2019, ''''),
    @(0x2018, ''''),
    @(0x201C, '"'),
    @(0x201D, '"'),
    @(0x2192, '->'),
    @(0x2190, '<-'),
    @(0x2265, '>='),
    @(0x2264, '<='),
    @(0x2260, '!='),
    @(0x00D7, 'x'),
    @(0x00B1, '+/-'),
    @(0x00B7, '.'),
    @(0x00A0, ' '),
    @(0x202F, ' '),
    @(0x2248, '~'),
    @(0x00A7, 'section '),
    @(0x21D2, '=>'),
    @(0x00B0, ' deg')
)
$utf8 = New-Object System.Text.UTF8Encoding $false
$files = Get-ChildItem crates -Recurse -Include *.rs, *.toml
$changed = 0
foreach ($f in $files) {
    $orig = [System.IO.File]::ReadAllText($f.FullName, $utf8)
    $t = $orig
    foreach ($p in $map) { $t = $t.Replace([string][char]$p[0], $p[1]) }
    $d = $t.Normalize([System.Text.NormalizationForm]::FormD)
    $sb = New-Object System.Text.StringBuilder
    foreach ($c in $d.ToCharArray()) {
        $cat = [System.Globalization.CharUnicodeInfo]::GetUnicodeCategory($c)
        if ($cat -ne [System.Globalization.UnicodeCategory]::NonSpacingMark) { [void]$sb.Append($c) }
    }
    $t = $sb.ToString().Normalize([System.Text.NormalizationForm]::FormC)
    if ($t -ne $orig) { [System.IO.File]::WriteAllText($f.FullName, $t, $utf8); $changed++ }
}
Write-Host "ascii.ps1 : $changed fichier(s) modifie(s)"
# Controle : il ne doit plus rester aucun caractere non ASCII.
$bad = $files | Select-String -Pattern '[^\x00-\x7F]'
if ($bad) { $bad | Select-Object -First 20; Write-Host "RESTE: caracteres non ASCII (a corriger a la main)"; exit 1 }
exit 0
