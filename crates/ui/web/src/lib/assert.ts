export const assertNever = (value: never): never => {
    throw new Error(`unreachable value: ${String(value)}`);
};
