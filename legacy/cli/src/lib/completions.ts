import type { Command } from "commander";

export function generateZshCompletions(program: Command): string {
    const commands = program.commands.map(cmd => `"${cmd.name()}:${(cmd.description() || "").replace(/"/g, '\\"')}"`).join("\n    ");
    return `#compdef fubbik
# Add to ~/.zshrc: eval "$(fubbik completions zsh)"

_fubbik_chunks() {
    local -a chunks
    chunks=(\${(f)"$(fubbik list -q 2>/dev/null | head -20)"})
    _describe 'chunk' chunks
}

_fubbik_templates() {
    local -a templates
    templates=(
        "convention:Convention template"
        "architecture-decision:Architecture Decision template"
        "runbook:Runbook template"
        "api-endpoint:API Endpoint template"
        "checklist:Checklist template"
    )
    _describe 'template' templates
}

_fubbik_spaces() {
    local -a spaces
    spaces=(\${(f)"$(fubbik space list -q 2>/dev/null | head -20)"})
    _describe 'space' spaces
}

_fubbik_open_targets() {
    local -a targets
    targets=(
        "dashboard:Overview dashboard"
        "graph:Knowledge graph visualization"
        "chunks:Chunk list"
        "requirements:Requirements page"
        "plans:Plans list"
        "import:Import page"
        "settings:Settings page"
        "health:Knowledge health"
        "tags:Tag management"
        "docs:API documentation"
    )
    _describe 'target' targets
}

_fubbik() {
    local -a commands
    commands=(
        ${commands}
    )

    _arguments -C \\
        '--json[output as JSON]' \\
        '-q[minimal output]' \\
        '--quiet[minimal output]' \\
        '1:command:->cmd' \\
        '*::arg:->args'

    case $state in
        cmd)
            _describe 'command' commands
            ;;
        args)
            case $words[1] in
                get|cat|update|remove)
                    _fubbik_chunks
                    ;;
                add)
                    _arguments \\
                        '--template[use template]:template:_fubbik_templates' \\
                        '--space[target space]:space:_fubbik_spaces' \\
                        '*:arg:'
                    ;;
                open)
                    _fubbik_open_targets
                    ;;
                list|search|export)
                    _arguments \\
                        '--space[target space]:space:_fubbik_spaces' \\
                        '*:arg:'
                    ;;
                *)
                    _arguments \\
                        '--space[target space]:space:_fubbik_spaces' \\
                        '*:arg:'
                    ;;
            esac
            ;;
    esac
}
compdef _fubbik fubbik`;
}
