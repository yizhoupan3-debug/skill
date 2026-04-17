import type { ErrorPayload } from './types.js';

/** Represents a recoverable or terminal runtime error for tool responses. */
export class BrowserToolError extends Error {
  public readonly code: string;
  public readonly recoverable: boolean;
  public readonly suggestedNextActions: string[];

  /**
   * Creates a browser tool error.
   * @param code Stable machine-readable error code.
   * @param message Human-readable error message.
   * @param recoverable Whether the agent can recover without human intervention.
   * @param suggestedNextActions Concrete next actions for recovery.
   */
  public constructor(
    code: string,
    message: string,
    recoverable = true,
    suggestedNextActions: string[] = [],
  ) {
    super(message);
    this.name = 'BrowserToolError';
    this.code = code;
    this.recoverable = recoverable;
    this.suggestedNextActions = suggestedNextActions;
  }

  /**
   * Converts the error into a JSON-safe payload.
   * @returns Serialized error payload for MCP tool responses.
   */
  public toPayload(): ErrorPayload {
    return {
      code: this.code,
      message: this.message,
      recoverable: this.recoverable,
      suggestedNextActions: this.suggestedNextActions,
    };
  }
}

/**
 * Normalizes unknown failures into a browser tool error.
 * @param error Unknown thrown value.
 * @returns Stable browser tool error.
 */
export function normalizeError(error: unknown): BrowserToolError {
  if (error instanceof BrowserToolError) {
    return error;
  }

  if (error instanceof Error) {
    return new BrowserToolError('UNEXPECTED_ERROR', error.message, true, ['inspect server logs']);
  }

  return new BrowserToolError('UNEXPECTED_ERROR', 'Unknown browser runtime failure.', true, [
    'inspect server logs',
  ]);
}
