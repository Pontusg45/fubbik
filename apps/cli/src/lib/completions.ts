import type { Command } from "commander";

export function generateZshCompletions(program: Command): string {
    const commands = program.commands
        .map((cmd) => `"${cmd.name()}:${(cmd.description() || "").replace(/"/g, '\\"')}"`)
        .join("\n    ");
    return `#compdef fubbik
# Add to ~/.zshrc: eval "$(fubbik completions zsh)"
_fubbik() {
  local -a commands
  commands=(
    ${commands}
  )
  _describe 'command' commands
}
compdef _fubbik fubbik`;
}
