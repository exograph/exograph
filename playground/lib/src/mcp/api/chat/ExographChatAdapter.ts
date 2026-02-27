// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { generateText, stepCountIs } from 'ai';
import type { ModelMessage, ToolCallPart, ToolModelMessage, TextPart } from '@ai-sdk/provider-utils';
import type { JSONValue } from '@ai-sdk/provider';
import type { ReadonlyJSONObject } from 'assistant-stream/utils';
import type { ChatModelAdapter, ChatModelRunOptions, ChatModelRunResult } from '@assistant-ui/react';
import type { ThreadMessage, ThreadAssistantMessagePart } from '@assistant-ui/react';
import type { MCPConnectionState } from '../client/MCPConnectionManager';
import { ModelId, type LLMProvider } from '../../providers/ModelId';
import { ModelAPI } from '../model/ModelAPI';
import { ToolErrorFactory, type ToolError } from '../../types/ToolError';

/**
 * Strip internal metadata (_toolError) from tool results before
 * sending them back to the AI SDK.
 */
function cleanToolResult(result: unknown): JSONValue {
  if (!result || typeof result !== 'object') return result as JSONValue;
  const { _toolError, ...clean } = result as Record<string, unknown>;
  return clean as JSONValue;
}

/**
 * Convert assistant-ui ThreadMessages to AI SDK message format.
 */
function convertMessages(messages: readonly ThreadMessage[]): ModelMessage[] {
  return messages.flatMap((msg): ModelMessage[] => {
    if (msg.role === 'system') {
      const textPart = msg.content.find(p => p.type === 'text');
      return textPart ? [{ role: 'system' as const, content: textPart.text }] : [];
    }

    if (msg.role === 'user') {
      const textParts = msg.content.filter(p => p.type === 'text');
      const text = textParts.map(p => p.text).join('');
      return text ? [{ role: 'user' as const, content: text }] : [];
    }

    if (msg.role === 'assistant') {
      const toolCallParts: ToolCallPart[] = [];
      const toolResults: ToolModelMessage[] = [];
      const textParts: TextPart[] = [];

      for (const part of msg.content) {
        if (part.type === 'text' && part.text) {
          textParts.push({ type: 'text', text: part.text });
        } else if (part.type === 'tool-call') {
          toolCallParts.push({
            type: 'tool-call',
            toolCallId: part.toolCallId,
            toolName: part.toolName,
            input: part.args,
          });
          if (part.result !== undefined) {
            toolResults.push({
              role: 'tool' as const,
              content: [{
                type: 'tool-result',
                toolCallId: part.toolCallId,
                toolName: part.toolName,
                output: { type: 'json' as const, value: cleanToolResult(part.result) },
              }],
            });
          }
        }
      }

      // AI SDK expects: assistant(tool-calls) -> tool(results) -> assistant(text)
      const converted: ModelMessage[] = [];
      if (toolCallParts.length > 0) {
        converted.push({ role: 'assistant' as const, content: toolCallParts });
        converted.push(...toolResults);
      }
      if (textParts.length > 0) {
        converted.push({ role: 'assistant' as const, content: textParts });
      }
      return converted;
    }

    return [];
  });
}

/**
 * Extract the actual tool output from a StaticToolResult wrapper.
 */
function unwrapToolOutput(toolResult: unknown): unknown {
  if (toolResult && typeof toolResult === 'object' && 'output' in toolResult) {
    return toolResult.output;
  }
  return toolResult;
}

/**
 * Analyze a tool output for errors.
 * Expects the unwrapped output (after unwrapToolOutput).
 */
function analyzeToolOutput(output: unknown): { hasError: boolean; toolError?: ToolError } {
  if (output == null) {
    return { hasError: true, toolError: ToolErrorFactory.fromUnknownError(null, 'Tool execution failed - no result returned') };
  }

  if (typeof output === 'object' && 'error' in output && (output as Record<string, unknown>).error) {
    return { hasError: true, toolError: ToolErrorFactory.parseError((output as Record<string, unknown>).error) };
  }

  // MCP format - check content array
  const obj = output as Record<string, unknown>;
  const contentArray = obj.value ?? obj.content;
  if (contentArray && Array.isArray(contentArray)) {
    for (const item of contentArray) {
      if (!item || typeof item !== 'object') continue;
      const entry = item as Record<string, unknown>;
      if (entry.error) {
        return { hasError: true, toolError: ToolErrorFactory.parseError(entry.error) };
      }
      if (typeof entry.text === 'string') {
        // Check for plain-text error messages from MCP tool results (e.g. "Error: Not authorized")
        if (entry.text.startsWith('Error: ')) {
          const errorMsg = entry.text;
          const isAuth = /not authorized|unauthorized|forbidden|access denied/i.test(errorMsg);
          const toolError = isAuth
            ? ToolErrorFactory.fromHttpError(403, errorMsg)
            : ToolErrorFactory.fromUnknownError(null, errorMsg);
          return { hasError: true, toolError };
        }
        try {
          const parsed = JSON.parse(entry.text);
          if (parsed.error) return { hasError: true, toolError: ToolErrorFactory.parseError(parsed.error) };
          if (parsed.status >= 400) return { hasError: true, toolError: ToolErrorFactory.fromHttpError(parsed.status, parsed.message) };
        } catch { /* not JSON */ }
      }
    }
  }

  return { hasError: false };
}

export interface ExographAdapterDeps {
  getApiKey: (provider: LLMProvider) => string | undefined;
  mcpState: MCPConnectionState;
  currentModel: ModelId;
}

function withToolError(toolOutput: unknown, toolError: ToolError): unknown {
  if (toolOutput && typeof toolOutput === 'object') {
    return { ...(toolOutput as Record<string, unknown>), _toolError: toolError };
  }
  return { _toolError: toolError, output: toolOutput };
}

export function createExographAdapter(depsRef: { current: ExographAdapterDeps }): ChatModelAdapter {
  const modelAPI = new ModelAPI();

  return {
    async *run({ messages, abortSignal }: ChatModelRunOptions): AsyncGenerator<ChatModelRunResult, void> {
      const { getApiKey, mcpState, currentModel } = depsRef.current;

      const apiKey = getApiKey(currentModel.provider) || '';
      const model = modelAPI.createModel(currentModel, apiKey, getApiKey);

      if (!model) {
        yield {
          content: [{ type: 'text' as const, text: 'Failed to create model - check your configuration' }],
          status: { type: 'incomplete' as const, reason: 'error' as const, error: 'Model creation failed' },
        };
        return;
      }

      const tools = mcpState.type === 'connected' ? mcpState.tools : undefined;
      const aiMessages = convertMessages(messages);

      let result;
      try {
        result = await generateText({
          model,
          messages: aiMessages,
          tools,
          stopWhen: stepCountIs(10),
          abortSignal,
        });
      } catch (error) {
        if (abortSignal?.aborted) return;
        const errorMessage = error instanceof Error ? error.message : 'Failed to generate response';
        yield {
          content: [{ type: 'text' as const, text: errorMessage }],
          status: { type: 'incomplete' as const, reason: 'error' as const, error: errorMessage },
        };
        return;
      }

      const allContent: ThreadAssistantMessagePart[] = [];

      for (const step of result.steps) {
        for (const toolCall of step.toolCalls) {
          const tcArgs = toolCall.input as ReadonlyJSONObject;
          const toolResult = step.toolResults?.find(
            (item) => item.toolCallId === toolCall.toolCallId
          );
          const toolOutput = toolResult != null ? unwrapToolOutput(toolResult) : undefined;
          const { hasError, toolError } = toolOutput !== undefined
            ? analyzeToolOutput(toolOutput)
            : { hasError: true, toolError: ToolErrorFactory.fromUnknownError(null, `Tool "${toolCall.toolName}" returned no result`) };

          allContent.push({
            type: 'tool-call' as const,
            toolCallId: toolCall.toolCallId,
            toolName: toolCall.toolName,
            args: tcArgs,
            argsText: JSON.stringify(tcArgs),
            result: hasError && toolError ? withToolError(toolOutput, toolError) : toolOutput,
            isError: hasError,
          });
        }
      }

      if (result.text) {
        allContent.push({ type: 'text' as const, text: result.text });
      }

      const hasErrors = allContent.some(p => p.type === 'tool-call' && 'isError' in p && p.isError);
      const isIncomplete = result.finishReason !== 'stop' && result.finishReason !== 'tool-calls';

      if (hasErrors || isIncomplete) {
        const errorMsg = hasErrors ? 'Tool call failed' : `Generation stopped: ${result.finishReason}`;
        yield {
          content: allContent,
          status: { type: 'incomplete' as const, reason: 'error' as const, error: errorMsg },
        };
      } else {
        yield {
          content: allContent,
          status: { type: 'complete' as const, reason: 'stop' as const },
        };
      }
    },
  };
}
