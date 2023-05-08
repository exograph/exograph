interface Info {
	id: number
	title: string
}

export async function getInfo(): Promise<Info> {
	return {
		id: 1,
		title: 'Test'
	}
}

