import type { Exograph, Operation } from '../generated/exograph.d.ts';

type Vector = number[]

const VECTOR_MAPPING = new Map<string, Vector>([
	["Car", [0.9, 0.8, 0.1]],
	["Motorcycle", [0.8, 0.5, 0.1]],
	["Dog", [0.1, 0.1, 0.9]],
	["Elephant", [0.6, 0.9, 0.9]],
	["Truck", [0.95, 0.85, 0.1]],
]);

function getEmbedding(content: string): number[] {
	return VECTOR_MAPPING.get(content) || [0.5, 0.5, 0.5];
}

const SEARCH_QUERY = `
		query($searchVector: [Float!]!) {
			documents(where: {contentVector: {similar: {value: $searchVector, distance: {lt: 0.5}}}}, orderBy: {contentVector: {value: $searchVector, order: ASC}}) {
				id
				title
				content
				contentVector
			}
		}`;

const UPDATE_EMBEDDING_MUTATION = `mutation($id: Int!, $contentVector: [Float!]!) { 
	updateDocument(id: $id, data: { contentVector: $contentVector }) {
		id
	}
}`;

export interface DocumentResult {
	id: number
	title: string
	content: string
	contentVector: Vector | null
}

export async function searchDocuments(searchString: string, exograph: Exograph): Promise<DocumentResult[]> {
	let embedding: number[] = getEmbedding(searchString);
	return (await exograph.executeQuery(SEARCH_QUERY, { searchVector: embedding })).documents;
}

export async function amendEmbedding(operation: Operation, exograph: Exograph) {
	const ret: { id: number } = await operation.proceed();

	const content: string = operation.query().arguments?.data?.content;
	let embedding = getEmbedding(content);

	await exograph.executeQuery(UPDATE_EMBEDDING_MUTATION, { id: ret.id, contentVector: embedding });

	return ret;
}

