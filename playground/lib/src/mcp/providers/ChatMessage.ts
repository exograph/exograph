// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

const MAX_TITLE_LENGTH = 50;

export const USER_ROLE = 'user' as const;
export const ASSISTANT_ROLE = 'assistant' as const;
export const SYSTEM_ROLE = 'system' as const;

export type MessageRole = typeof USER_ROLE | typeof ASSISTANT_ROLE | typeof SYSTEM_ROLE;

export class ChatMessage {
  id: string;
  role: MessageRole;
  content: string;
  timestamp: Date;

  constructor(
    id: string,
    role: MessageRole,
    content: string,
    timestamp: Date = new Date()
  ) {
    this.id = id;
    this.role = role;
    this.content = content;
    this.timestamp = timestamp;
  }

  generateTitle(): string {
    const cleaned = this.content.replace(/\s+/g, ' ').trim();
    return cleaned.length <= MAX_TITLE_LENGTH
      ? cleaned
      : `${cleaned.substring(0, MAX_TITLE_LENGTH - 3)}...`;
  }

  static fromJSON(data: any): ChatMessage {
    return new ChatMessage(
      data.id,
      data.role,
      data.content,
      new Date(data.timestamp)
    );
  }

  toJSON() {
    return {
      id: this.id,
      role: this.role,
      content: this.content,
      timestamp: this.timestamp
    };
  }
}