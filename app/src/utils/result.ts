import type { AppError } from "@/lib/bindings.gen";

/** What a command answers: the Rust side's `IpcResult`. */
export type Result<T, E = AppError> = { ok: true; value: T } | { ok: false; error: E };

/** The command's value, or its error thrown, as TanStack Query takes a failure. */
export function unwrap<T, E>(result: Result<T, E>): T {
  if (!result.ok) {
    throw result.error;
  }
  return result.value;
}

/** A command as a `queryFn`. */
export function queryFn<T, E>(command: () => Promise<Result<T, E>>): () => Promise<T> {
  return async () => unwrap(await command());
}

/** A command with one argument as a `mutationFn`. */
export function mutationFn<T, E, V>(
  command: (variables: V) => Promise<Result<T, E>>,
): (variables: V) => Promise<T> {
  return async (variables) => unwrap(await command(variables));
}

/** Whether a thrown value is an error a command answered. */
export function isAppError(value: unknown): value is AppError {
  return typeof value === "object" && value !== null && "code" in value;
}
