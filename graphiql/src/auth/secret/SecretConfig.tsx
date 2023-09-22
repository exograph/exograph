const defaultPayload = `{
  "sub": "1234567890",
  "name": "Jordan Taylor"
}`;

export class SecretConfig {
  private _secret: string;
  get secret(): string {
    return this._secret;
  }

  private _payload: string;
  get payload(): string {
    return this._payload;
  }

  constructor(secret: string, payload: string) {
    this._secret = secret;
    this._payload = payload;
  }

  canSignIn(): boolean {
    return !!this.secret && !!this.payload;
  }

  static loadConfig(): SecretConfig {
    return new SecretConfig("", defaultPayload);
  }
}
