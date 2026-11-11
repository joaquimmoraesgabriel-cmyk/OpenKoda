param(
    [Parameter(Mandatory)][int]$AppProcessId,
    [Parameter(Mandatory)][ValidateSet('folder', 'file')][string]$Selection,
    [string]$FixturePath
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class PickerFixtureControls {
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    public static extern IntPtr SendMessageTimeout(IntPtr window, uint message, IntPtr param, string value, uint flags, uint timeout, out IntPtr result);
    [DllImport("user32.dll")]
    public static extern bool PostMessage(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);
    [DllImport("user32.dll")]
    public static extern bool IsChild(IntPtr parent, IntPtr child);
    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr window);
}
'@

$uia = [System.Windows.Automation.AutomationElement]
$tree = [System.Windows.Automation.TreeScope]
function Condition($Property, $Value) {
    [System.Windows.Automation.PropertyCondition]::new($Property, $Value)
}
function Wait-ForElement([scriptblock]$Find, [string]$Description) {
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        $value = & $Find
        if ($value) { return $value }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Controle indisponivel no dialogo de teste: $Description."
}

$owned = Condition $uia::ProcessIdProperty $AppProcessId
$title = if ($Selection -eq 'folder') { 'Escolher pasta do projeto' } else { 'Escolher arquivo do projeto' }
$dialogMatch = [System.Windows.Automation.AndCondition]::new($owned,
    (Condition $uia::ClassNameProperty '#32770'), (Condition $uia::NameProperty $title))
$dialog = Wait-ForElement {
    foreach ($root in $uia::RootElement.FindAll($tree::Children, $owned)) {
        $match = $root.FindFirst($tree::Subtree, $dialogMatch)
        if ($match) { return $match }
    }
} $title
$windowHandle = [IntPtr]$dialog.Current.NativeWindowHandle
function Get-Control([string]$Class, [string[]]$Ids) {
    foreach ($id in $Ids) {
        $match = [System.Windows.Automation.AndCondition]::new((Condition $uia::ClassNameProperty $Class),
            (Condition $uia::AutomationIdProperty $id))
        $control = $dialog.FindFirst($tree::Descendants, $match)
        if (!$control -or $control.Current.IsOffscreen) { continue }
        $handle = [IntPtr]$control.Current.NativeWindowHandle
        if ($control.Current.ProcessId -eq $AppProcessId -and [PickerFixtureControls]::IsChild($windowHandle, $handle)) {
            return $handle
        }
    }
}
function Set-PathText([string]$Text) {
    # The Shell exposes the name field as a Win32 Edit, sometimes as a UIA Pane.
    $edit = Wait-ForElement { Get-Control 'Edit' @('1148','1152','1001') } 'caminho'
    $result = [IntPtr]::Zero
    $sent = [PickerFixtureControls]::SendMessageTimeout($edit, 0x000C, [IntPtr]::Zero, $Text, 2, 3000, [ref]$result)
    if ($sent -eq [IntPtr]::Zero -or $result -eq [IntPtr]::Zero) { throw 'O campo recusou o caminho da fixture.' }
}
function Invoke-DialogButton([int]$Id) {
    $button = Wait-ForElement { Get-Control 'Button' @([string]$Id) } 'botao'
    if (![PickerFixtureControls]::PostMessage($windowHandle, 0x0111, [IntPtr]$Id, $button)) {
        throw 'Nao foi possivel acionar o botao do dialogo de teste.'
    }
}

if (!$FixturePath) {
    Invoke-DialogButton 2
} else {
    $target = (Resolve-Path -LiteralPath $FixturePath).Path
    Set-PathText $target
    Invoke-DialogButton 1
    if ($Selection -eq 'folder') {
        # On some Windows Shell versions the first confirmation navigates into the
        # typed folder. Select the current folder only after its breadcrumb is visible.
        $leaf = Split-Path -Leaf $target
        $state = Wait-ForElement {
            if (![PickerFixtureControls]::IsWindowVisible($windowHandle)) { return 'closed' }
            $bar = $dialog.FindFirst($tree::Descendants, (Condition $uia::ClassNameProperty 'Breadcrumb Parent'))
            if ($bar -and ($bar.FindFirst($tree::Descendants, (Condition $uia::NameProperty $leaf)) -or
                $bar.FindAll($tree::Descendants, [System.Windows.Automation.Condition]::TrueCondition).Where({
                    $_.Current.Name.EndsWith($target, [StringComparison]::OrdinalIgnoreCase)
                }).Count)) { return 'at-target' }
        } 'pasta da fixture'
        if ($state -eq 'at-target' -and [PickerFixtureControls]::IsWindowVisible($windowHandle)) {
            Set-PathText ''
            Invoke-DialogButton 1
        }
    }
}
[void](Wait-ForElement { ![PickerFixtureControls]::IsWindowVisible($windowHandle) } 'fechamento')
