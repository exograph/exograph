// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

/**
 * An abstraction layer for localStorage operations with serialization and error handling
 */
export class StorageAPI {
  static getItem<T>(key: string): T | null {
    try {
      const stored = localStorage.getItem(key);
      if (!stored) return null;
      return JSON.parse(stored) as T;
    } catch (error) {
      console.error(`Error loading from localStorage key "${key}":`, error);
      return null;
    }
  }

  static setItem<T>(key: string, value: T): void {
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch (error) {
      console.error(`Error saving to localStorage key "${key}":`, error);
    }
  }

  static removeItem(key: string): void {
    try {
      localStorage.removeItem(key);
    } catch (error) {
      console.error(`Error removing localStorage key "${key}":`, error);
    }
  }

  static hasItem(key: string): boolean {
    try {
      return localStorage.getItem(key) !== null;
    } catch (error) {
      console.error(`Error checking localStorage key "${key}":`, error);
      return false;
    }
  }

  static getItemWithFallback<T>(key: string, fallback: T): T {
    const value = this.getItem<T>(key);
    return value !== null ? value : fallback;
  }
}