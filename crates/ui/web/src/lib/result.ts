export type Result<T, E = unknown> = { ok: true; value: T } | { ok: false; error: E };

export const ok = <T>(value: T): Result<T, never> => ({ ok: true, value });

export const err = <E>(error: E): Result<never, E> => ({ ok: false, error });

export const attempt = <T, E = unknown>(operation: () => T): Result<T, E> => {
    try {
        return ok(operation());
    } catch (error) {
        return err(error as E);
    }
};

export const attemptAsync = async <T, E = unknown>(
    operation: () => Promise<T>,
): Promise<Result<T, E>> => {
    try {
        return ok(await operation());
    } catch (error) {
        return err(error as E);
    }
};
