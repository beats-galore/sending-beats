/**
 * Read a human-readable message out of anything a rejected call throws.
 *
 * Tauri commands reject with the plain `String` their Rust side returned, not an
 * `Error`, so an `error instanceof Error` check discards every backend message
 * and leaves the user with "Unknown error" while the real cause is thrown away.
 */
export const describeError = (error: unknown, fallback = 'Unknown error'): string => {
  if (typeof error === 'string') {
    return error.trim() === '' ? fallback : error;
  }

  if (error instanceof Error) {
    return error.message.trim() === '' ? fallback : error.message;
  }

  if (error !== null && typeof error === 'object' && 'message' in error) {
    const { message } = error as { message: unknown };
    if (typeof message === 'string' && message.trim() !== '') {
      return message;
    }
  }

  return fallback;
};
