# Load this file in the current shell to enable `loom use` and `loom clear`.
# Compatible with Bash and Zsh. Loading it does not activate an environment.
loom() {
    case "${1-}" in
        use|clear)
            local __loom_code
            __loom_code="$(command "${LOOM_EXECUTABLE:-loom}" "$@")" || return $?
            eval "$__loom_code"
            ;;
        *)
            command "${LOOM_EXECUTABLE:-loom}" "$@"
            ;;
    esac
}
