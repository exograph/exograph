import { updateLocalStorage } from "../../AuthContext";

const clerkPublishableKey = "exograph:clerkPublishableKey";
const clerkTemplateIdKey = "exograph:clerkTemplateId";

export class ClerkConfig {
  private _publishableKey?: string;
  get publishableKey(): string | undefined {
    return this._publishableKey;
  }

  private _templateId?: string;
  get templateId(): string | undefined {
    return this._templateId;
  }

  constructor(publishableKey?: string, templateId?: string) {
    this._publishableKey = publishableKey;
    this._templateId = templateId;
  }

  canSignIn(): boolean {
    return !!this.publishableKey;
  }

  saveConfig() {
    updateLocalStorage(clerkPublishableKey, this.publishableKey);
    updateLocalStorage(clerkTemplateIdKey, this.templateId);
  }

  static loadConfig(): ClerkConfig {
    return new ClerkConfig(
      localStorage.getItem(clerkPublishableKey) || undefined,
      localStorage.getItem(clerkTemplateIdKey) || undefined
    );
  }
}
