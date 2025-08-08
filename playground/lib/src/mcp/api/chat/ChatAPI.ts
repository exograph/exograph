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
  args: any;
  result: any;
}

export interface ChatResponseWithSteps {
  text: string;
  toolCalls: ToolCallInfo[];
}

export interface ChatGenerationOptions {
  maxSteps?: number;
  tools?: Record<string, any>;
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
          toolCalls.push({
            toolName: toolCall.toolName,
            toolCallId: toolCall.toolCallId,
            args: toolCall.input,
            result: toolResult ? toolResult.output : undefined,
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