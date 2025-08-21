// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { BaseMessage } from '../../providers/ChatMessage';
import { ModelId } from '../../providers/ModelId';
import { generateId } from '../../utils/idGenerator';

export class ChatConversation {
  id: string;
  title: string;
  messages: BaseMessage[];
  updatedAt: Date;
  isScratchPad: boolean;

  constructor(
    isScratchPad: boolean = true,
    id?: string,
    title?: string,
    messages?: BaseMessage[],
    updatedAt?: Date
  ) {
    this.id = id || generateId();
    this.title = title || 'New Conversation';
    this.messages = messages || [];
    this.updatedAt = updatedAt || new Date();
    this.isScratchPad = isScratchPad;
  }

  addMessage(message: BaseMessage): ChatConversation {
    return new ChatConversation(
      this.messages.length === 0 ? this.isScratchPad : false,
      this.id,
      message.generateTitle(),
      [...this.messages, message],
      new Date()
    );
  }

  get currentModelId(): ModelId | null {
    if (this.messages.length === 0) return null;
    return this.messages[this.messages.length - 1].modelId;
  }
}