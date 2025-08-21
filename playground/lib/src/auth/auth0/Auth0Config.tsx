import { updateLocalStorage } from "../AuthContext";

const auth0DomainKey = "exograph:auth0DomainKey";
const auth0ClientIdKey = "exograph:auth0ClientIdKey";
const auth0ProfileKey = "exograph:auth0ProfileId";

export class Auth0Config {
  private _domain?: string;
  get domain(): string | undefined {
    return this._domain;
  }

  private _clientId?: string;
  get clientId(): string | undefined {
    return this._clientId;
  }

  _profile: string | undefined;
  get profile(): string | undefined {
    return this._profile;
  }

  constructor(domain?: string, clientId?: string, profile?: string) {
    this._domain = domain;
    this._clientId = clientId;
    this._profile = profile;
  }

  canSignIn(): boolean {
    return !!this.clientId && !!this.domain && !!this.profile;
  }

  saveConfig() {
    updateLocalStorage(auth0DomainKey, this.domain);
    updateLocalStorage(auth0ClientIdKey, this.clientId);
    updateLocalStorage(auth0ProfileKey, this.profile);
  }

  static loadConfig(): Auth0Config {
    return new Auth0Config(
      localStorage.getItem(auth0DomainKey) || undefined,
      localStorage.getItem(auth0ClientIdKey) || undefined,
      localStorage.getItem(auth0ProfileKey) || undefined
    );
  }
}
