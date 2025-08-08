// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { StorageAPI } from './StorageAPI';
import { LLMProvider } from '../../providers/ModelId';

const STORAGE_KEY = 'exograph::mcp::playground::api-keys';

export class ProviderStorage {
  static loadApiKeys(): Record<LLMProvider, string> {
    return StorageAPI.getItemWithFallback<Record<LLMProvider, string>>(
      STORAGE_KEY,
      {} as Record<LLMProvider, string>
    );
  }

  static saveApiKeys(keys: Record<LLMProvider, string>): void {
    // Only save non-empty keys
    const keysToStore = Object.fromEntries(
      Object.entries(keys).filter(([, key]) => key && key.trim())
    );

    if (Object.keys(keysToStore).length > 0) {
      StorageAPI.setItem(STORAGE_KEY, keysToStore);
    } else {
      StorageAPI.removeItem(STORAGE_KEY);
    }
  }
}