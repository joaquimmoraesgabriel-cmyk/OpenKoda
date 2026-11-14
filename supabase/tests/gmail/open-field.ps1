$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[Windows.Forms.Application]::EnableVisualStyles()
$form = New-Object Windows.Forms.Form
$form.Text = 'OpenKoda | Teste de entrega no Gmail'
$form.StartPosition = 'CenterScreen'
$form.ClientSize = New-Object Drawing.Size(460,191)
$form.FormBorderStyle = 'FixedDialog'
$form.MaximizeBox = $false
$form.MinimizeBox = $false
$form.TopMost = $true
$form.ShowInTaskbar = $true
$label = New-Object Windows.Forms.Label
$label.Location = New-Object Drawing.Point(18,16)
$label.Size = New-Object Drawing.Size(423,58)
$label.Text = 'Insira uma chave temporária do Resend com permissão apenas de envio. Ela será usada pelo servidor local para enviar dois e-mails de teste ao seu Gmail.'
$box = New-Object Windows.Forms.TextBox
$box.Location = New-Object Drawing.Point(18,83)
$box.Width = 423
$box.UseSystemPasswordChar = $true
$ok = New-Object Windows.Forms.Button
$ok.Text = 'Iniciar teste'
$ok.Location = New-Object Drawing.Point(220,132)
$ok.Width = 111
$ok.DialogResult = 'OK'
$cancel = New-Object Windows.Forms.Button
$cancel.Text = 'Cancelar'
$cancel.Location = New-Object Drawing.Point(337,132)
$cancel.Width = 104
$cancel.DialogResult = 'Cancel'
$form.Controls.AddRange(@($label,$box,$ok,$cancel))
$form.AcceptButton = $ok
$form.CancelButton = $cancel
$form.Add_Shown({
  $form.Activate()
  $form.BringToFront()
  [void]$box.Focus()
  [Console]::Error.WriteLine('O campo protegido do OpenKoda foi aberto.')
})
if ($form.ShowDialog() -ne 'OK') { $form.Dispose(); exit 2 }
$value = $box.Text.Trim()
$box.Clear()
$form.Dispose()
if ($value -notmatch '^re_[a-zA-Z0-9_-]{12,180}$') { exit 3 }
# Only the parent process reads stdout, over an anonymous pipe.
[Console]::WriteLine($value)
$value = $null
