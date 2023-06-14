interface Info {
	id: number
	title: string
}

export function getInfo(): Info {
	return {
		id: 1,
		title: 'Test'
	}
}

