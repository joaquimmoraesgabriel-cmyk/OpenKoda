param([switch]$Offline,[switch]$BundleStage)
$ErrorActionPreference = 'Stop'
$appRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$source = Join-Path $appRoot 'security-runtime'
$local = New-Item -ItemType Directory -Path (Join-Path $appRoot '.local') -Force
if ($local.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'O destino de preparo foi redirecionado.' }
$stage = (New-Item -ItemType Directory -Path (Join-Path $local.FullName ('security-runtime-stage-'+[guid]::NewGuid()))).FullName
$node = (Get-Command node.exe -ErrorAction Stop).Source
$nodeVersion = & $node -p process.versions.node
$nodeArch = & $node -p process.arch
$v = [version]$nodeVersion
if ($LASTEXITCODE -ne 0 -or $nodeArch -ne 'x64' -or ($v.Major -notin @(24,26) -and -not ($v.Major -eq 22 -and $v.Minor -ge 13))) { throw 'Use Node.js x64 22.13+, 24 ou 26 para preparar o instalador x64.' }
$py = @(& python -I -B -c 'import sys,platform; print(sys.base_prefix); print(sys.version.split()[0]); print(platform.machine())')
if ($LASTEXITCODE -ne 0 -or $py.Count -ne 3 -or [version]$py[1] -lt [version]'3.11' -or $py[2] -notin @('AMD64','x86_64')) { throw 'Python x64 3.11 ou posterior é necessário para preparar o instalador.' }
$pyRoot = $py[0]

# Keep development dependencies, tests, local output and user files out of the bundle.
foreach ($name in @('package.json','package-lock.json','cli.mjs','worker.mjs','scan.mjs','platform.mjs','README.md')) {
    $inputFile = Get-Item -LiteralPath (Join-Path $source $name)
    if ($inputFile.PSIsContainer -or $inputFile.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Recurso fonte do motor inválido.' }
    Copy-Item -LiteralPath $inputFile.FullName -Destination $stage
}
Push-Location -LiteralPath $stage
try {
    $npmArgs = @('ci','--ignore-scripts','--omit=dev','--include=optional','--no-audit','--no-fund')
    if ($Offline) { $npmArgs += '--offline' }
    & npm.cmd @npmArgs | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'Não foi possível restaurar o SDK oficial pelo lockfile.' }
    New-Item -ItemType Directory -Path 'bin','python','python/Lib','python/DLLs' | Out-Null
    Copy-Item -LiteralPath $node -Destination 'bin/node.exe'
    $license = @('LICENSE','LICENSE.txt') | ForEach-Object { Join-Path (Split-Path $node) $_ } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
    if ($license) { Copy-Item -LiteralPath $license -Destination 'bin/LICENSE.txt' }
    elseif ($Offline) {
        $oldNode = Join-Path $source 'bin/node.exe'
        $oldNotice = Join-Path $source 'bin/LICENSE.txt'
        if (!(Test-Path -LiteralPath $oldNode -PathType Leaf) -or !(Test-Path -LiteralPath $oldNotice -PathType Leaf) -or (Get-FileHash $oldNode).Hash -ne (Get-FileHash $node).Hash) { throw 'O modo offline precisa do aviso do mesmo executável Node.js.' }
        Copy-Item -LiteralPath $oldNotice -Destination 'bin/LICENSE.txt'
    } else { Invoke-WebRequest -Uri "https://raw.githubusercontent.com/nodejs/node/v$nodeVersion/LICENSE" -OutFile 'bin/LICENSE.txt' }
    Copy-Item -LiteralPath (Join-Path $pyRoot 'LICENSE.txt') -Destination 'python/LICENSE.txt'
    Get-ChildItem -LiteralPath $pyRoot -File | Where-Object { $_.Name -eq 'python.exe' -or $_.Name -match '^(python3\d*|vcruntime\d+(_\d+)?)\.dll$' } | Copy-Item -Destination 'python'
    & robocopy.exe (Join-Path $pyRoot 'Lib') (Join-Path $stage 'python/Lib') /S /XJ /R:0 /W:0 /NFL /NDL /NJH /NJS /NP /XD site-packages __pycache__ test tests idlelib tkinter ensurepip /XF '*.pyc' '*.pyo' | Out-Null
    if ($LASTEXITCODE -ge 8) { throw 'Falha ao preparar a biblioteca padrão Python.' }
    Get-ChildItem -LiteralPath (Join-Path $pyRoot 'DLLs') -File | Where-Object { $_.Extension -in @('.dll','.pyd') -and $_.Name -notlike '*tk*' -and $_.Name -notlike '*tcl*' } | Copy-Item -Destination 'python/DLLs'
    $pythonPrefix = & ./python/python.exe -I -B -c 'import sys,ssl,sqlite3,tomllib,ctypes,subprocess; print(sys.base_prefix)'
    if ($LASTEXITCODE -ne 0 -or [IO.Path]::GetFullPath($pythonPrefix) -ne (Join-Path $stage 'python')) { throw 'O Python empacotado não localizou suas bibliotecas no pacote.' }
    $diagnostic = & ./bin/node.exe cli.mjs --check
    $code = $LASTEXITCODE
    $check = $diagnostic | ConvertFrom-Json
    if ($code -ne 0 -or !$check.available -or $check.nodeVersion -ne $nodeVersion -or $check.pythonVersion -ne $py[1]) { throw ('Componentes indisponíveis no pacote: '+($check.missing -join ', ')) }
    if (Get-ChildItem -LiteralPath $stage -Recurse -Force -Attributes ReparsePoint | Select-Object -First 1) { throw 'O runtime contém caminho redirecionado.' }
    $inventory = & ./bin/node.exe (Join-Path $PSScriptRoot 'security-runtime-inventory.mjs') $stage
    if ($LASTEXITCODE -ne 0) { throw 'Falha ao verificar a árvore do runtime.' }
    $result = [pscustomobject]@{ directory=$stage; diagnostic=$check; inventory=($inventory | ConvertFrom-Json) }
} finally { Pop-Location }
if (!$BundleStage) {
    # The development entry points expect these three generated directories beside cli.mjs.
    if ((Get-Item -LiteralPath $source).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'A pasta de desenvolvimento foi redirecionada.' }
    $backup = (New-Item -ItemType Directory -Path (Join-Path $stage 'previous')).FullName
    $saved = @(); $published = @()
    try {
        foreach ($name in @('bin','python','node_modules')) {
            $destination = Join-Path $source $name
            $incoming = Get-Item -LiteralPath (Join-Path $stage $name)
            if ($incoming.Parent.FullName -ne $stage -or $incoming.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Destino de desenvolvimento inválido.' }
            if (Test-Path -LiteralPath $destination) {
                $previous = Get-Item -LiteralPath $destination
                if (!$previous.PSIsContainer -or $previous.Parent.FullName -ne $source -or $previous.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Pasta de desenvolvimento inválida.' }
                Move-Item -LiteralPath $previous.FullName -Destination (Join-Path $backup $name)
                $saved += $name
            }
            Move-Item -LiteralPath $incoming.FullName -Destination $destination
            $published += $name
        }
    } catch {
        foreach ($name in $published) { Move-Item -LiteralPath (Join-Path $source $name) -Destination (Join-Path $stage $name) }
        foreach ($name in $saved) { Move-Item -LiteralPath (Join-Path $backup $name) -Destination (Join-Path $source $name) }
        throw
    }
    $result.directory = $source
    # This directory contains only the prior generated dependencies, now replaced as a whole.
    $owned = Get-Item -LiteralPath $stage
    if ($owned.Parent.FullName -ne $local.FullName -or $owned.Name -notmatch '^security-runtime-stage-[0-9a-f-]{36}$' -or $owned.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Pasta temporária inesperada.' }
    if (Get-ChildItem -LiteralPath $stage -Force -Recurse -Attributes ReparsePoint | Select-Object -First 1) { Write-Warning "A cópia anterior contém um caminho redirecionado e foi preservada em $stage." }
    else { Remove-Item -LiteralPath $owned.FullName -Recurse -Force }
}
$result
