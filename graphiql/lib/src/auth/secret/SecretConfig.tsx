const defaultClaims = `{
  "sub": "1234567890",
  "name": "Jordan Taylor"
}`;

export class JwtSecret {
  constructor(readonly value: string, readonly readOnly: boolean) {
    this.value = value;
    this.readOnly = readOnly;
  }

  static default(): JwtSecret {
    return new JwtSecret("", false);
  }
}

export class SecretConfig {
  constructor(readonly secret: JwtSecret, readonly claims: string) {
    this.secret = secret;
    this.claims = claims;
  }

  updated(value: string, claims: string): SecretConfig {
    return new SecretConfig(new JwtSecret(value, this.secret.readOnly), claims);
  }

  canSignIn(): boolean {
    return !!this.secret && !!this.claims;
  }

  static loadConfig(secret: JwtSecret): SecretConfig {
    return new SecretConfig(secret || JwtSecret.default(), defaultClaims);
  }
}
