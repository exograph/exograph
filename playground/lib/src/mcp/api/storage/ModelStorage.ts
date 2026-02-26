// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { StorageAPI } from './StorageAPI';
import { ModelId } from '../../providers/ModelId';
import { ModelSettings } from '../../context/ModelContext';

const CURRENT_MODEL = 'exograph::playground::mcp::model::current';
const MODEL_SETTINGS = 'exograph::playground::mcp::model::settings';

export class ModelStorage {
  static loadCurrentModel(): { provider: string; model: string } | null {
    return StorageAPI.getItem(CURRENT_MODEL);
  }

  static saveCurrentModel(modelId: ModelId): void {
    StorageAPI.setItem(CURRENT_MODEL, {
      provider: modelId.provider,
      model: modelId.model,
    });
  }

  static loadModelSettings(): Record<string, ModelSettings> {
    return StorageAPI.getItemWithFallback(
      MODEL_SETTINGS,
      {}
    );
  }

  static saveModelSettings(settings: Record<string, ModelSettings>): void {
    StorageAPI.setItem(MODEL_SETTINGS, settings);
  }
}