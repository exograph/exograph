import type { Exograph, Operation } from '../generated/exograph.d.ts';

import type { AdminEnvContext, AdminHeaderContext, AuthContext } from '../generated/contexts.d.ts';

const auditMutation = `
	mutation createAudit($operation: String!, $result: String!, $authContext: String!, $adminHeaderContext: String!, $adminEnvContext: String!){
		createAudit(data: {operation: $operation, result: $result, authContext:$authContext, adminHeaderContext: $adminHeaderContext, adminEnvContext: $adminEnvContext}) {
			id
		}
	}`;

export async function captureContext(operation: Operation, authContext: AuthContext, adminHeaderContext: AdminHeaderContext, adminEnvContext: AdminEnvContext, exograph: Exograph) {
	const result = await operation.proceed();

	let operation_str = JSON.stringify(operation.query());
	let result_str = JSON.stringify(result);

	let authContext_str = JSON.stringify(authContext);
	let adminHeaderContext_str = JSON.stringify(adminHeaderContext);
	let adminEnvContext_str = JSON.stringify(adminEnvContext);

	await exograph.executeQuery(auditMutation, {
		operation: operation_str,
		result: result_str,
		authContext: authContext_str,
		adminHeaderContext: adminHeaderContext_str,
		adminEnvContext: adminEnvContext_str
	});

	return result;
}

