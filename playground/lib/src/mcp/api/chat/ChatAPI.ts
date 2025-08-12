// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { generateText, stepCountIs } from 'ai';
import type { LanguageModel } from 'ai';
import { BaseMessage } from '../../providers/ChatMessage';

const DEFAULT_MAX_STEPS = 10;

export interface ToolCallInfo {
  toolName: string;
  toolCallId?: string;
  args: Record<string, unknown>;
  result: {
    error?: any;
    status?: number;
    output?: any;
    [key: string]: any;
  } | null;
}

export interface ChatResponseWithSteps {
  text: string;
  toolCalls: ToolCallInfo[];
}

export interface ChatGenerationOptions {
  maxSteps?: number;
  tools?: Record<string, any>;
}

// Type guard for checking if an object has an error property
function hasError(obj: unknown): obj is { error: unknown } {
  return typeof obj === 'object' && obj !== null && 'error' in obj;
}

// Type guard for step content with tool-error type
function isToolErrorContent(item: unknown): item is { type: 'tool-error'; error: { status?: number; message?: string; name?: string } } {
  return typeof item === 'object' && item !== null && 
         'type' in item && (item as any).type === 'tool-error' &&
         'error' in item && typeof (item as any).error === 'object';
}

export class ChatAPI {
  /**
   * Extract tool calls from generation steps
   */
  private static extractToolCalls(steps: any[]): ToolCallInfo[] {
    const toolCalls: ToolCallInfo[] = [];

    for (const step of steps) {
      if (step.toolCalls && step.toolCalls.length > 0) {
        for (let i = 0; i < step.toolCalls.length; i++) {
          const toolCall = step.toolCalls[i];
          const toolResult = step.toolResults?.[i];

          // Handle case where toolResult is undefined (error case)
          let result = toolResult;
          if (!toolResult) {
            // When tool execution fails, AI SDK might not provide toolResult
            // Check if this step has any error information we can extract

            // Try to extract error information from the step
            let errorMessage = 'Tool execution failed';
            let status = undefined;

            // Look for tool-error in the step content
            if (step.content && Array.isArray(step.content)) {
              const toolError = step.content.find(isToolErrorContent);
              if (toolError) {
                if (toolError.error.status) {
                  status = toolError.error.status;
                }

                // Update error message if available
                if (toolError.error.message) {
                  errorMessage = toolError.error.message;
                } else if (toolError.error.name) {
                  errorMessage = `${toolError.error.name}: HTTP error! status: ${status}`;
                }
              }
            }

            result = {
              error: errorMessage,
              ...(status !== undefined ? { status } : {})
            };
          } else if (hasError(toolResult)) {
            result = {
              ...toolResult,
              error: toolResult.error
            };
          }

          toolCalls.push({
            toolName: toolCall.toolName,
            toolCallId: toolCall.toolCallId,
            args: toolCall.input,
            result: result,
          });
        }
      }
    }

    return toolCalls;
  }

  /**
   * Generate chat response with optional tools
   */
  static async generateChatResponse(
    messages: BaseMessage[],
    model: LanguageModel,
    options: ChatGenerationOptions = {}
  ): Promise<ChatResponseWithSteps> {
    const { maxSteps = DEFAULT_MAX_STEPS, tools } = options;

    try {
      const modelMessages = messages.map(msg => msg.toModelMessage());

      const result = await generateText({
        model,
        messages: modelMessages,
        tools,
        stopWhen: stepCountIs(maxSteps),
      });

      // Extract tool calls from all steps
      const toolCalls = result.steps ? this.extractToolCalls(result.steps) : [];

      return {
        text: result.text,
        toolCalls,
      };
    } catch (error) {
      console.error('Error in generateChatResponse:', error);
      throw new Error(
        error instanceof Error
          ? error.message
          : 'Failed to generate chat response'
      );
    }
  }
}