export async function getAuthenticatedSecret(): Promise<string> {
	return 'authenticated-secret';
}

export async function setAuthenticatedSecret(secret: string): Promise<string> {
	return secret.toUpperCase();
}

export async function getUnauthenticatedSecret(): Promise<string> {
	return 'unauthenticated-secret';
}

export async function setUnauthenticatedSecret(secret: string): Promise<string> {
	return secret.toUpperCase();
}

export async function getAdminSecret(): Promise<string> {
	return 'admin-secret';
}

