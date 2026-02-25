import argparse
import argcomplete

parser = argparse.ArgumentParser(description='quasi-agent completion')
parser.add_argument('shell', choices=['bash', 'zsh', 'fish'])
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
