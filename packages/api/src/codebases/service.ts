// Legacy alias for the VS Code extension. Re-exports from ../spaces/service
// with the old codebase-* names mapped to the new space-* implementations.
export {
    listSpaces as listCodebases,
    getSpace as getCodebase,
    createSpace as createCodebase,
    updateSpace as updateCodebase,
    deleteSpace as deleteCodebase,
    resetSpace as resetCodebase,
    detectSpace as detectCodebase
} from "../spaces/service";
