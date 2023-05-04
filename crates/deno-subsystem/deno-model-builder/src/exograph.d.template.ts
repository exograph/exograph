// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

export interface Exograph {
  executeQuery(query: string, variable?: { [key: string]: any }): Promise<any>;
  addResponseHeader(name: string, value: string): Promise<void>;
  setCookie(cookie: {
    name: string,
    value: string,
    expires: Date,
    maxAge: number,
    domain: string,
    path: string,
    secure: boolean,
    httpOnly: boolean,
    sameSite: "Lax" | "Strict" | "None"
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

export declare class ExographError extends Error {
  constructor(message: string);
}