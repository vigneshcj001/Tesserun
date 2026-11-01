param([ValidateSet('run','server','check','test','build','lint','format')][string]$Action = 'run')
# Optional adapter. rust.cmd also works when PowerShell blocks this script.
& (Join-Path $PSScriptRoot 'rust.cmd') $Action
exit $LASTEXITCODE
