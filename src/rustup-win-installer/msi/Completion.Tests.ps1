if (Get-Command -Name rustup -ErrorAction Stop) {
	# Enable rustup tab completion
	rustup completions powershell | Out-String | Invoke-Expression
}

function Get-Result([string]$Text, [int]$CursorPosition = $Text.Length, [hashtable]$Options) {
	[System.Management.Automation.CommandCompletion]::CompleteInput($Text, $CursorPosition, $Options).CompletionMatches
}

Describe 'rustup flags and subcommands' {
	$result = Get-Result 'rustup '
	$result.Count | Should Be 23
	$result[00].CompletionText | Should Be 'completions'
	$result[01].CompletionText | Should Be 'component'
	$result[02].CompletionText | Should Be 'default'
	$result[03].CompletionText | Should Be 'doc'
	$result[04].CompletionText | Should Be '-h'
	$result[05].CompletionText | Should Be 'help'
	$result[06].CompletionText | Should Be '--help'
	$result[07].CompletionText | Should Be 'install'
	$result[08].CompletionText | Should Be 'override'
	$result[09].CompletionText | Should Be 'run'
	$result[10].CompletionText | Should Be 'self'
	$result[11].CompletionText | Should Be 'set'
	$result[12].CompletionText | Should Be 'show'
	$result[13].CompletionText | Should Be 'target'
	$result[14].CompletionText | Should Be 'telemetry'
	$result[15].CompletionText | Should Be 'toolchain'
	$result[16].CompletionText | Should Be 'uninstall'
	$result[17].CompletionText | Should Be 'update'
	$result[18].CompletionText | Should Be '-V'
	$result[19].CompletionText | Should Be '-v'
	$result[20].CompletionText | Should Be '--verbose'
	$result[21].CompletionText | Should Be '--version'
	$result[22].CompletionText | Should Be 'which'
}
