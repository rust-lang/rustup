if (Get-Command -Name rustup -ErrorAction Stop) {
	# Enable rustup tab completion
	rustup completions powershell | Out-String | Invoke-Expression
}

function Get-Result([string]$Text, [int]$CursorPosition = $Text.Length, [hashtable]$Options) {
	[System.Management.Automation.CommandCompletion]::CompleteInput($Text, $CursorPosition, $Options).CompletionMatches
}

Describe 'rustup flags and subcommands' {
	$result = Get-Result 'rustup '

	# Keep it simple because over-specify testing might be risky
	$result.Count | Should -BeGreaterThan 10
	$result.CompletionText | Should -Contain 'completions'
	$result.CompletionText | Should -Contain 'install'
	$result.CompletionText | Should -Contain 'uninstall'
	$result.CompletionText | Should -Contain 'update'
	$result.CompletionText | Should -Contain '--help'
	$result.CompletionText | Should -Contain '--verbose'
}
