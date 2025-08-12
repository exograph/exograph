// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

export const OPENAI_PROVIDER = 'openai' as const;
export const ANTHROPIC_PROVIDER = 'anthropic' as const;
export const GOOGLE_PROVIDER = 'google' as const;

export type LLMProvider = typeof OPENAI_PROVIDER | typeof ANTHROPIC_PROVIDER | typeof GOOGLE_PROVIDER;

// Model identifier combining provider and model
export class ModelId {
  constructor(
    public readonly provider: LLMProvider,
    public readonly model: string
  ) {}

  toString(): string {
    return `${this.provider}:${this.model}`;
  }

  equals(other: ModelId): boolean {
    return this.provider === other.provider && this.model === other.model;
  }

  static create(provider: LLMProvider, model: string): ModelId {
    return new ModelId(provider, model);
  }
}