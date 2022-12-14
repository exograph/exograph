interface Claytip {
  executeQuery(query: string, variable?: { [key: string]: any }): Promise<any>;
  addResponseHeader(name: string, value: string ): Promise<void>;
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

interface ClaytipPriv extends Claytip {
  executeQueryPriv(query: string, variable?: { [key: string]: any }, contextOverride?: { [key: string]: any }): Promise<any>;
}

type JsonObject = { [Key in string]?: JsonValue };
type JsonValue = string | number | boolean | null | JsonObject | JsonValue[];

interface Field {
    alias: string | null;
    name: string;
    arguments: JsonObject;
    subfields: Field[];
}

interface Operation {
    name(): string;
    proceed<T>(): Promise<T>;
    query(): Field;
}

declare class ClaytipError extends Error {
    constructor(message: string);
}