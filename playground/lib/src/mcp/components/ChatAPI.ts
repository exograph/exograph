// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { generateText, streamText } from 'ai';
import type { LanguageModel } from 'ai';
import { ChatConfig, ChatMessage, MessageRole } from '../providers/types';

export interface ChatAPIRequest {
  messages: { role: MessageRole; content: string }[];
  config: ChatConfig;
}

export interface ChatAPIResponse {
  content: string;
  error?: string;
}

export async function sendChatMessage(
  messages: ChatMessage[],
  model: LanguageModel
): Promise<AsyncIterable<string>> {
  try {
    const formattedMessages = messages.map(msg => ({
      role: msg.role,
      content: msg.content,
    }));

    const result = await streamText({
      model,
      messages: formattedMessages,
    });

    return result.textStream;
  } catch (error) {
    console.error('Error in sendChatMessage:', error);
    throw error;
  }
}

export async function generateChatResponse(
  messages: ChatMessage[],
  model: LanguageModel
): Promise<string> {
  try {
    const formattedMessages = messages.map(msg => ({
      role: msg.role,
      content: msg.content,
    }));

    const result = await generateText({
      model,
      messages: formattedMessages,
    });

    return result.text;
  } catch (error) {
    console.error('Error in generateChatResponse:', error);
    throw error;
  }
}