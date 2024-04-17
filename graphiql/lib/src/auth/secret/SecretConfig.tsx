const defaultClaims = `{
  "sub": "1234567890",
  "name": "Jordan Taylor"
}`;

export class SecretConfig {
  private _secret: string;
  get secret(): string {
    return this._secret;
  }

  private _claims: string;
  get claims(): string {
    return this._claims;
  }

  constructor(secret: string, claims: string) {
    this._secret = secret;
    this._claims = claims;
  }

  canSignIn(): boolean {
    return !!this.secret && !!this.claims;
  }

  static loadConfig(): SecretConfig {
    return new SecretConfig("", defaultClaims);
  }
}
