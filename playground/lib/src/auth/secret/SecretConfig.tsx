const defaultClaims = `{
  "sub": "1234567890",
  "name": "Jordan Taylor"
}`;

const STORAGE_KEY = "exograph_playground_auth_profiles_v1";

function generateProfileId(): string {
  if (
    typeof crypto !== "undefined" &&
    typeof (crypto as { randomUUID?: () => string }).randomUUID === "function"
  ) {
    return crypto.randomUUID();
  }
  return `profile-${Date.now()}-${Math.floor(Math.random() * 1_000_000)}`;
}

export type AuthProfileMode = "generated" | "static";

export class JwtSecret {
  constructor(readonly value: string, readonly readOnly: boolean) {
    this.value = value;
    this.readOnly = readOnly;
  }

  static default(): JwtSecret {
    return new JwtSecret("", false);
  }
}

export interface AuthProfile {
  id: string;
  name: string;
  mode: AuthProfileMode;
  sharedSecret?: string;
  rawToken?: string;
  claims: string;
  headers: Record<string, string>;
  readOnly?: boolean;
}

interface SecretConfigState {
  activeProfileId: string;
  profiles: AuthProfile[];
}

function createDefaultProfile(secret: JwtSecret | undefined): AuthProfile {
  return {
    id: "default",
    name: "Default",
    mode: "generated",
    sharedSecret: secret?.value ?? "",
    readOnly: secret?.readOnly,
    claims: defaultClaims,
    headers: {},
  };
}

export class SecretConfig {
  readonly secret: JwtSecret;
  readonly claims: string;
  readonly headers: Record<string, string>;
  readonly mode: AuthProfileMode;

  private constructor(
    readonly state: SecretConfigState,
    readonly fallbackSecret: JwtSecret,
  ) {
    const profile = this.activeProfile;
    this.mode = profile.mode;

    if (profile.mode === "generated") {
      this.secret = new JwtSecret(profile.sharedSecret ?? "", !!profile.readOnly);
      this.claims = profile.claims ?? defaultClaims;
    } else {
      this.secret = new JwtSecret(profile.rawToken ?? "", false);
      this.claims = profile.claims ?? "{}";
    }

    this.headers = profile.headers ?? {};
  }

  static loadConfig(secret?: JwtSecret): SecretConfig {
    const baseSecret = secret ?? JwtSecret.default();
    let parsed: SecretConfigState | undefined;

    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) {
        parsed = SecretConfig.normalize(JSON.parse(stored), baseSecret);
      }
    } catch (e) {
      console.warn("Failed to load auth profiles from localStorage:", e);
    }

    if (!parsed) {
      parsed = {
        activeProfileId: "default",
        profiles: [createDefaultProfile(secret)],
      };
    }

    return new SecretConfig(parsed, baseSecret);
  }

  private static normalize(
    raw: any,
    secret: JwtSecret
  ): SecretConfigState | undefined {
    if (!raw || typeof raw !== "object") {
      return undefined;
    }

    const profiles = Array.isArray(raw.profiles)
      ? (raw.profiles as any[])
          .map<AuthProfile | null>((profile, index) => {
            if (!profile || typeof profile !== "object") {
              return null;
            }
            const id = typeof profile.id === "string" ? profile.id : `profile-${index}`;
            const name =
              typeof profile.name === "string" && profile.name.trim()
                ? profile.name.trim()
                : `Profile ${index + 1}`;
            const mode: AuthProfileMode =
              profile.mode === "static" ? "static" : "generated";

            const claims =
              typeof profile.claims === "string" && profile.claims.trim()
                ? profile.claims
                : defaultClaims;

            const headers =
              profile.headers && typeof profile.headers === "object"
                ? { ...profile.headers }
                : {};

            const sharedSecret =
              typeof profile.sharedSecret === "string"
                ? profile.sharedSecret
                : mode === "generated"
                  ? secret.value
                  : undefined;

            const rawToken =
              typeof profile.rawToken === "string" ? profile.rawToken : "";

            return {
              id,
              name,
              mode,
              sharedSecret,
              rawToken,
              claims,
              headers,
              readOnly: !!profile.readOnly,
            };
          })
          .filter((p): p is AuthProfile => p !== null)
      : [];

    if (!profiles.length) {
      profiles.push(createDefaultProfile(secret));
    }

    const active =
      typeof raw.activeProfileId === "string" &&
      profiles.some((p) => p.id === raw.activeProfileId)
        ? raw.activeProfileId
        : profiles[0].id;

    return {
      activeProfileId: active,
      profiles,
    };
  }

  private get activeProfile(): AuthProfile {
    const profile =
      this.state.profiles.find((p) => p.id === this.state.activeProfileId) ??
      this.state.profiles[0] ??
      createDefaultProfile(this.fallbackSecret);
    return profile;
  }

  get profiles(): AuthProfile[] {
    return [...this.state.profiles];
  }

  get activeProfileId(): string {
    return this.activeProfile.id;
  }

  withActiveProfile(profileId: string): SecretConfig {
    if (!this.state.profiles.some((p) => p.id === profileId)) {
      return this;
    }

    return this.withState({
      ...this.state,
      activeProfileId: profileId,
    });
  }

  withUpdatedActiveProfile(update: Partial<AuthProfile>): SecretConfig {
    return this.withProfiles(
      this.state.profiles.map((profile) =>
        profile.id === this.activeProfile.id
          ? {
              ...profile,
              ...update,
            }
          : profile
      )
    );
  }

  addProfile(profile: Partial<AuthProfile>): SecretConfig {
    const id = profile.id ?? generateProfileId();
    const name = profile.name?.trim() || "New Profile";
    const mode: AuthProfileMode = profile.mode ?? "static";

    const newProfile: AuthProfile = {
      id,
      name,
      mode,
      sharedSecret:
        mode === "generated"
          ? profile.sharedSecret ?? this.fallbackSecret.value
          : undefined,
      rawToken: mode === "static" ? profile.rawToken ?? "" : profile.rawToken,
      claims: profile.claims ?? defaultClaims,
      headers: profile.headers ?? {},
      readOnly: profile.readOnly ?? false,
    };

    return this.withState({
      activeProfileId: id,
      profiles: [...this.state.profiles, newProfile],
    });
  }

  removeProfile(profileId: string): SecretConfig {
    if (this.state.profiles.length <= 1) {
      return this;
    }

    const filtered = this.state.profiles.filter(
      (profile) => profile.id !== profileId
    );

    const nextActive =
      profileId === this.state.activeProfileId
        ? filtered[0]?.id ?? filtered[filtered.length - 1].id
        : this.state.activeProfileId;

    return this.withState({
      activeProfileId: nextActive,
      profiles: filtered,
    });
  }

  updated(
    value: string,
    claims: string,
    headers?: Record<string, string>
  ): SecretConfig {
    if (this.mode === "static") {
      return this.withUpdatedActiveProfile({
        rawToken: value,
        headers: headers ?? this.headers,
      });
    }

    return this.withUpdatedActiveProfile({
      sharedSecret: value,
      claims,
      headers: headers ?? this.headers,
    });
  }

  updateHeaders(headers: Record<string, string>): SecretConfig {
    return this.withUpdatedActiveProfile({ headers });
  }

  updateClaims(claims: string): SecretConfig {
    return this.withUpdatedActiveProfile({ claims });
  }

  updateSharedSecret(secret: string): SecretConfig {
    return this.withUpdatedActiveProfile({ sharedSecret: secret });
  }

  updateToken(token: string): SecretConfig {
    return this.withUpdatedActiveProfile({ rawToken: token });
  }

  renameActiveProfile(name: string): SecretConfig {
    return this.withUpdatedActiveProfile({ name: name.trim() || "Profile" });
  }

  setActiveMode(mode: AuthProfileMode): SecretConfig {
    const profile = this.activeProfile;
    if (profile.mode === mode) {
      return this;
    }

    if (mode === "generated") {
      return this.withUpdatedActiveProfile({
        mode,
        sharedSecret: this.fallbackSecret.value,
      });
    }

    return this.withUpdatedActiveProfile({
      mode,
      rawToken: profile.rawToken ?? "",
    });
  }

  canSignIn(): boolean {
    if (this.mode === "static") {
      return !!this.secret.value;
    }

    return !!this.secret.value && !!this.claims;
  }

  save(): void {
    try {
      const payload: SecretConfigState = {
        activeProfileId: this.state.activeProfileId,
        profiles: this.state.profiles.map((profile) => ({
          ...profile,
        })),
      };

      localStorage.setItem(STORAGE_KEY, JSON.stringify(payload));
    } catch (e) {
      console.warn("Failed to save auth profiles to localStorage:", e);
    }
  }

  private withProfiles(profiles: AuthProfile[]): SecretConfig {
    const activeId = profiles.some((p) => p.id === this.state.activeProfileId)
      ? this.state.activeProfileId
      : profiles[0]?.id ?? "default";

    return this.withState({
      activeProfileId: activeId,
      profiles,
    });
  }

  private withState(state: SecretConfigState): SecretConfig {
    return new SecretConfig(state, this.fallbackSecret);
  }
}
