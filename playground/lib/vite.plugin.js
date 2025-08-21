import $monacoEditorPlugin from 'vite-plugin-monaco-editor'

const monacoEditorPlugin = $monacoEditorPlugin.default ?? $monacoEditorPlugin

export function createMonacoEditorPlugin() {
  return monacoEditorPlugin({
    languageWorkers: ['editorWorkerService', 'json'],
    customWorkers: [
      {
        label: 'graphql',
        entry: 'monaco-graphql/esm/graphql.worker.js'
      }
    ]
  })
}