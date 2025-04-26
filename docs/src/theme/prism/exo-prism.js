Prism.languages.exo = {
	'keyword': /\b(?:context|type|enum|module|interceptor|query|mutation|self)\b/,

	'number': /-?\b\d+(?:\.\d+)?(?:e[+-]?\d+)?\b/i,
	'string': {
		pattern: /(^|[^\\])"(?:\\.|[^\\"\r\n])*"(?!\s*:)/,
		lookbehind: true,
		greedy: true
	},
	'boolean': /\b(?:false|true)\b/,

	'type-name': {
		pattern: /(\b(?:type|context|enum)\s+)\w+/i,
		lookbehind: true,
	},
	'module-name': {
		pattern: /(\b(?:module)\s+)\w+/i,
		lookbehind: true,
	},
	'operation-name': {
		pattern: /(\b(?:query|mutation)\s+)\w+/i,
		lookbehind: true,
	},

	'field-access': {
		pattern: /(?:[.]\s*\w+)|(?:\w+\s*[.]\s*\w+)/,
	},

	'property': {
		pattern: /(\w+)\s*:/i,
	},
	'builtin': /\b(?:Array|Set|Int|Float|Boolean|String)\b/,
	'comment': {
		pattern: /\/\/.*|\/\*[\s\S]*?(?:\*\/|$)/,
		greedy: true
	},
	'operator': {
		pattern: /(==)|(\!=)|(<=)(>=)|(<)|(>)|(\|\|)|(&&)|(\!)/
	},
	'annotation': /(@{1}\w*\(.*?\))|(@{1}\w+)/i,
	'punctuation': /[{}[\],]/,
}
