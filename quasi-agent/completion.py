import argparse
import argcomplete

parser = argparse.ArgumentParser(description='quasi-agent completion')
parser.add_argument('shell', choices=['bash', 'zsh', 'fish', 'powershell'])
argcomplete.autocomplete(parser)

args = parser.parse_args()

if args.shell == 'bash':
    print('# quasi-agent bash completion start')
    print('complete -o default -F _quasi_agent quasi-agent')
    print('# quasi-agent bash completion end')
elif args.shell == 'zsh':
    print('# quasi-agent zsh completion start')
    print('compdef _quasi_agent quasi-agent')
    print('# quasi-agent zsh completion end')
elif args.shell == 'fish':
    print('# quasi-agent fish completion start')
    print('complete -c quasi-agent -f')
    print('# quasi-agent fish completion end')
elif args.shell == 'powershell':
    print('# quasi-agent PowerShell completion start')
    print('Register-ArgumentCompleter -CommandName quasi-agent -ScriptBlock {')
    print('    param($commandName, $wordToComplete, $cursorPosition)')
    print('    @("list", "claim", "complete", "watch", "ledger", "contributors", "verify") |')
    print('        Where-Object { $_ -like "$wordToComplete*" } |')
    print('        ForEach-Object {')
    print('            [System.Management.Automation.CompletionResult]::new($_, $_, "ParameterValue", $_)')
    print('        }')
    print('}')
    print('# quasi-agent PowerShell completion end')
