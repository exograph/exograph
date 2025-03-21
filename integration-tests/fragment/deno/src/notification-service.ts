import type { Exograph } from '../generated/exograph.d.ts';

import type { Notification } from '../generated/NotificationService.d.ts';

export async function getNotifications(exograph: Exograph): Promise<Notification[]> {
	let { todos } = await exograph.executeQuery(`
		{
			todos {
				id
				title
				completed
			}
		}
	`);

	return todos;
}

