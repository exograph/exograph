/*
 * Copyright Exograph, Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

export interface Exograph {
  executeQuery(query: string, variable?: { [key: string]: any }): Promise<any>;
  addResponseHeader(name: string, value: string): Promise<void>;
  setCookie(cookie: {
    name: string,
    value: string,
    expires?: Date,
    maxAge?: number,
    domain?: string,
    path?: string,
    secure?: boolean,
    httpOnly?: boolean,
    sameSite?: "Lax" | "Strict" | "None"
  }): Promise<void>;
}

export interface ExographPriv extends Exograph {
  executeQueryPriv(query: string, variable?: { [key: string]: any }, contextOverride?: { [key: string]: any }): Promise<any>;
}

export type JsonObject = { [Key in string]?: JsonValue };
export type JsonValue = string | number | boolean | null | JsonObject | JsonValue[];

export interface Field {
  alias: string | null;
  name: string;
  arguments: JsonObject;
  subfields: Field[];
}

export interface Operation {
  name(): string;
  proceed<T>(): Promise<T>;
  query(): Field;
}

declare global {
  class ExographError extends Error {
    constructor(message: string);
  }
}