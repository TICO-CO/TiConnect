export type Result<T, E = unknown> = [value: T | null, err: E | null];

export const attempt = <T, E = unknown>(operation: () => T): Result<T, E> => {
    try {
        return [operation(), null];
    } catch (error) {
        return [null, error as E];
    }
};

export const attemptAsync = async <T, E = unknown>(
    operation: () => Promise<T>,
): Promise<Result<T, E>> => {
    try {
        return [await operation(), null];
    } catch (error) {
        return [null, error as E];
    }
};
