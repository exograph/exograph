import type { Message, Question } from '../generated/ChatService.d.ts';

export function chat(messages: Message[]): string {
	return messages.map((message) => {
		return message.text.toUpperCase();
	}).join(' ');
}

export function generateQuestions(_projectId: string): Question[] {
	return ["What is your name?", "How can I help you?"].map((text) => {
		return {
			content: text
		};
	});
}

export function initialQuestion(_projectId: string): Question {
	return { content: "What's up?" };
}

