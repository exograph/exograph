// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import type {
  SystemModelMessage,
  UserModelMessage,
  AssistantModelMessage,
  ToolModelMessage,
} from 'ai';
import { ModelId } from './ModelId';

export type ModelMessage = SystemModelMessage | UserModelMessage | AssistantModelMessage | ToolModelMessage;
export type MessageRole = 'user' | 'assistant' | 'system' | 'tool';

export abstract class BaseMessage<M extends ModelMessage = ModelMessage> {
  id: string;
  timestamp: Date;
  content: M;
  modelId: ModelId;

  protected constructor(id: string, content: M, modelId: ModelId, timestamp: Date = new Date()) {
    this.id = id;
    this.content = content;
    this.modelId = modelId;
    this.timestamp = timestamp;
  }

  get role(): MessageRole {
    return this.content.role;
  }

  abstract generateTitle(): string;

  /**
   * Convert message to format expected by AI model
   * Default implementation returns the content as-is
   * Specific message types can override for custom formatting
   */
  toModelMessage(): M | UserModelMessage {
    return this.content;
  }

  toJSON() {
    return {
      id: this.id,
      timestamp: this.timestamp,
      role: this.role,
      content: this.content,
      modelId: this.modelId
    };
  }
}

// Abstract base for text-based messages (User, Assistant, System)
export abstract class BaseTextMessage<M extends UserModelMessage | AssistantModelMessage | SystemModelMessage> extends BaseMessage<M> {
  generateTitle(): string {
    const MAX_TITLE_LENGTH = 50;
    const contentStr = typeof this.content.content === 'string' ? this.content.content : 'Complex content';
    const cleaned = contentStr.replace(/\s+/g, ' ').trim();
    return cleaned.length <= MAX_TITLE_LENGTH
      ? cleaned
      : `${cleaned.substring(0, MAX_TITLE_LENGTH - 3)}...`;
  }
}

export class UserMessage extends BaseTextMessage<UserModelMessage> {
  constructor(id: string, content: string, modelId: ModelId, timestamp: Date = new Date()) {
    super(id, { role: 'user', content }, modelId, timestamp);
  }
}

export class AssistantMessage extends BaseTextMessage<AssistantModelMessage> {
  constructor(id: string, content: string, modelId: ModelId, timestamp: Date = new Date()) {
    super(id, { role: 'assistant', content }, modelId, timestamp);
  }
}

export class SystemMessage extends BaseTextMessage<SystemModelMessage> {
  constructor(id: string, content: string, modelId: ModelId, timestamp: Date = new Date()) {
    super(id, { role: 'system', content }, modelId, timestamp);
  }
}

export class ToolMessage extends BaseMessage<ToolModelMessage> {
  private _args: any;

  constructor(
    id: string,
    toolName: string,
    toolCallId: string,
    args: any,
    result: any,
    modelId: ModelId,
    timestamp: Date = new Date()
  ) {
    const toolContent: ToolModelMessage = {
      role: 'tool',
      content: [
        {
          type: 'tool-result',
          toolCallId: toolCallId,
          toolName: toolName,
          output: result
        }
      ]
    };

    super(id, toolContent, modelId, timestamp);
    this._args = args;
  }

  get toolName(): string {
    const toolResult = this.content.content[0] as any;
    return toolResult?.toolName || '';
  }

  get toolCallId(): string {
    const toolResult = this.content.content[0] as any;
    return toolResult?.toolCallId || '';
  }

  get args(): any {
    return this._args;
  }

  get result(): any {
    const toolResult = this.content.content[0] as any;
    return toolResult?.output;
  }

  generateTitle(): string {
    return `Tool: ${this.toolName}`;
  }

  /**
   * Override to convert tool messages to user messages for Gemini compatibility
   * See https://ai.google.dev/gemini-api/docs/function-calling?example=chart#step-4
   * (Gemini passes back tool result with the `user` role)
   */
  toModelMessage(): UserModelMessage {
    return {
      role: 'user',
      content: JSON.stringify(this.content),
    };
  }

  toJSON() {
    return {
      ...super.toJSON(),
      toolName: this.toolName,
      toolCallId: this.toolCallId,
      args: this.args,
      result: this.result
    };
  }
}


export function isUserMessage(message: BaseMessage): message is UserMessage {
  return message instanceof UserMessage;
}

export function isAssistantMessage(message: BaseMessage): message is AssistantMessage {
  return message instanceof AssistantMessage;
}

export function isSystemMessage(message: BaseMessage): message is SystemMessage {
  return message instanceof SystemMessage;
}

export function isToolMessage(message: BaseMessage): message is ToolMessage {
  return message instanceof ToolMessage;
}

export function deserializeMessage(data: any): BaseMessage {
  if (!data || !data.id || !data.role || !data.content || !data.modelId) {
    throw new Error('Invalid message data structure');
  }

  const timestamp = new Date(data.timestamp);
  const modelId = ModelId.create(data.modelId.provider, data.modelId.model);

  switch (data.role) {
    case 'user':
      return new UserMessage(data.id, data.content.content, modelId, timestamp);

    case 'assistant':
      return new AssistantMessage(data.id, data.content.content, modelId, timestamp);

    case 'system':
      return new SystemMessage(data.id, data.content.content, modelId, timestamp);

    case 'tool':
      const toolResult = data.content.content[0];
      return new ToolMessage(
        data.id,
        toolResult.toolName,
        toolResult.toolCallId,
        data.args,
        toolResult.output,
        modelId,
        timestamp
      );

    default:
      throw new Error(`Unknown message role: ${data.role}`);
  }
}